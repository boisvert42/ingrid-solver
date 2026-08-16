import init, { WasmSolver } from "../pkg/ingrid_core.js";

let solver = null;
let cellNames = [];
let slotConfigs = [];
let activeSlotId = null;
let remainingOptions = [];
let dictContents = "";
let activeCandidates = [];
let currentValidationTask = null;

// DOM Elements
const initBtn = document.getElementById("init-btn");
const statusDiv = document.getElementById("status");
const slotsInput = document.getElementById("slots-input");
const slotsList = document.getElementById("slots-list");
const cellsBoard = document.getElementById("cells-board");
const candidatesList = document.getElementById("candidates-list");
const uploadBtn = document.getElementById("upload-btn");
const dictFileInput = document.getElementById("dict-file-input");
const uploadedDictName = document.getElementById("uploaded-dict-name");
const minScoreInput = document.getElementById("min-score-input");

// Initialize Wasm module
async function run() {
    try {
        statusDiv.textContent = "Status: Downloading dictionary...";
        const response = await fetch("../resources/spreadthewordlist.dict");
        if (!response.ok) {
            throw new Error(`Failed to load dictionary: ${response.statusText}`);
        }
        dictContents = await response.text();
        
        statusDiv.textContent = "Status: Loading WebAssembly...";
        await init();
        
        statusDiv.textContent = "Status: Ready! Click 'Initialize Solver'.";
        initBtn.disabled = false;
        
        // Auto-initialize the default grid
        initializeSolver();
    } catch (e) {
        statusDiv.textContent = `Status: Error initializing: ${e.message}`;
        console.error(e);
    }
}

// Initialize Solver instance
function initializeSolver() {
    if (!dictContents) {
        statusDiv.textContent = "Status: Dictionary not loaded yet.";
        return;
    }
    
    try {
        statusDiv.textContent = "Status: Initializing solver...";
        
        const slotsDef = slotsInput.value;
        
        // 1. Create solver instance
        solver = new WasmSolver(slotsDef);
        
        // 2. Configure min score
        const minScore = parseInt(minScoreInput.value) || 0;
        solver.set_min_score(minScore);
        
        // 3. Load dictionary
        solver.load_dictionary(dictContents);
        
        // 4. Get cell names
        cellNames = JSON.parse(solver.get_cell_names());
        
        // 5. Parse slots configuration to display
        parseSlotsConfiguration(slotsDef);
        
        // 6. Render Board & Slots List
        renderBoard();
        renderSlotsList();
        
        // 7. Propagate initial constraints
        propagateConstraints();
        
        statusDiv.textContent = "Status: Solver Initialized.";
    } catch (e) {
        statusDiv.textContent = `Status: Error: ${e}`;
        console.error(e);
    }
}

// Parse slots string for UI mapping
function parseSlotsConfiguration(slotsDef) {
    const lines = slotsDef.includes(';') ? slotsDef.split(';') : slotsDef.split('\n');
    slotConfigs = [];
    let id = 0;
    
    for (const line of lines) {
        const cells = line.replace(/;/g, '').trim().split(/\s+/).filter(x => x.length > 0);
        if (cells.length > 0) {
            slotConfigs.push({
                id,
                cells,
                length: cells.length
            });
            id++;
        }
    }
}

// Render cells on the board by word slots
function renderBoard() {
    cellsBoard.innerHTML = "";
    
    slotConfigs.forEach(slot => {
        const row = document.createElement("div");
        row.className = "board-row";
        row.id = `board-row-${slot.id}`;
        
        const label = document.createElement("span");
        label.className = "board-row-label";
        label.textContent = `Slot ${slot.id + 1}`;
        
        const inputsContainer = document.createElement("div");
        inputsContainer.className = "board-row-inputs";
        
        slot.cells.forEach((cellName, charIdx) => {
            const square = document.createElement("div");
            square.className = "cell-square";
            square.dataset.cell = cellName;
            
            const cellLabel = document.createElement("span");
            cellLabel.className = "cell-square-name";
            cellLabel.textContent = cellName;
            
            const input = document.createElement("input");
            input.className = "cell-square-input";
            input.type = "text";
            input.maxLength = 1;
            input.dataset.cell = cellName;
            input.dataset.slotId = slot.id;
            input.dataset.charIdx = charIdx;
            
            // Sync values across all cells sharing this name
            input.addEventListener("input", (e) => {
                const val = e.target.value.toLowerCase().replace(/[^a-z]/g, '');
                
                // Find all inputs sharing the same cell name and sync them
                document.querySelectorAll(`input[data-cell="${cellName}"]`).forEach(inp => {
                    inp.value = val;
                });
                
                propagateConstraints();

                // Auto-advance to the next cell in the slot if we typed a character
                if (val.length > 0 && charIdx < slot.cells.length - 1) {
                    const nextInput = document.querySelector(
                        `input[data-slot-id="${slot.id}"][data-char-idx="${charIdx + 1}"]`
                    );
                    if (nextInput) {
                        nextInput.focus();
                    }
                }
            });

            // Backspace navigation
            input.addEventListener("keydown", (e) => {
                if (e.key === "Backspace") {
                    e.preventDefault();
                    
                    // Clear cell value
                    input.value = "";
                    document.querySelectorAll(`input[data-cell="${cellName}"]`).forEach(inp => {
                        inp.value = "";
                    });
                    
                    propagateConstraints();
                    
                    // Navigate to previous cell in the same slot
                    if (charIdx > 0) {
                        const prevInput = document.querySelector(
                            `input[data-slot-id="${slot.id}"][data-char-idx="${charIdx - 1}"]`
                        );
                        if (prevInput) {
                            prevInput.focus();
                        }
                    }
                }
            });
            
            // Highlight crossings on focus
            input.addEventListener("focus", () => {
                selectSlot(slot.id); // Clicking/focusing an input selects its slot!
                
                document.querySelectorAll(".cell-square").forEach(sq => {
                    sq.classList.remove("crossing-highlight");
                });
                document.querySelectorAll(`.cell-square[data-cell="${cellName}"]`).forEach(sq => {
                    sq.classList.add("crossing-highlight");
                });
            });
            
            input.addEventListener("blur", () => {
                document.querySelectorAll(".cell-square").forEach(sq => {
                    sq.classList.remove("crossing-highlight");
                });
            });
            
            square.appendChild(cellLabel);
            square.appendChild(input);
            inputsContainer.appendChild(square);
        });
        
        row.appendChild(label);
        row.appendChild(inputsContainer);
        
        // Clicking the row selects the slot
        row.addEventListener("click", (e) => {
            if (e.target.tagName !== "INPUT") {
                selectSlot(slot.id);
            }
        });
        
        cellsBoard.appendChild(row);
    });
}

// Render word slots list in left panel
function renderSlotsList() {
    slotsList.innerHTML = "";
    
    slotConfigs.forEach(slot => {
        const item = document.createElement("div");
        item.className = "slot-item";
        item.dataset.id = slot.id;
        item.id = `slot-item-${slot.id}`;
        
        const nameSpan = document.createElement("span");
        nameSpan.className = "slot-name";
        nameSpan.textContent = `Slot ${slot.id + 1}: ${slot.cells.join(" ")}`;
        
        const lengthSpan = document.createElement("span");
        lengthSpan.className = "slot-length";
        lengthSpan.id = `slot-len-badge-${slot.id}`;
        lengthSpan.textContent = `${slot.length} letters`;
        
        item.appendChild(nameSpan);
        item.appendChild(lengthSpan);
        
        item.addEventListener("click", () => {
            selectSlot(slot.id);
        });
        
        slotsList.appendChild(item);
    });
}

// Select a word slot
function selectSlot(slotId) {
    activeSlotId = slotId;
    
    // Update active class in slots list
    document.querySelectorAll(".slot-item").forEach(item => {
        item.classList.remove("active");
    });
    const slotItem = document.getElementById(`slot-item-${slotId}`);
    if (slotItem) slotItem.classList.add("active");
    
    // Update active class in board rows
    document.querySelectorAll(".board-row").forEach(row => {
        row.classList.remove("active");
    });
    const boardRow = document.getElementById(`board-row-${slotId}`);
    if (boardRow) boardRow.classList.add("active");
    
    // Update active candidates list (which triggers rendering and validation)
    updateActiveCandidates(remainingOptions[slotId] || []);
}

// Read current board fill state
function getBoardFill() {
    const fill = {};
    cellNames.forEach(name => {
        const inp = document.querySelector(`input[data-cell="${name}"]`);
        const val = inp ? inp.value : "";
        if (val && val.length > 0) {
            fill[name] = val;
        }
    });
    return fill;
}

// Run AC-3 constraint propagation
function propagateConstraints() {
    if (!solver) return;
    
    try {
        const fill = getBoardFill();
        const resultsJson = solver.run_ac3(JSON.stringify(fill));
        remainingOptions = JSON.parse(resultsJson);
        
        // Update slot badges with option counts
        slotConfigs.forEach(slot => {
            const count = remainingOptions[slot.id].length;
            const badge = document.getElementById(`slot-len-badge-${slot.id}`);
            badge.textContent = `${count} options`;
            if (count === 0) {
                badge.style.backgroundColor = "#ef4444"; // red alert
                badge.style.color = "white";
            } else if (count === 1) {
                badge.style.backgroundColor = "#10b981"; // green fixed
                badge.style.color = "white";
            } else {
                badge.style.backgroundColor = "";
                badge.style.color = "";
            }
        });
        
        // Update active candidates list
        if (activeSlotId !== null) {
            updateActiveCandidates(remainingOptions[activeSlotId] || []);
        }
    } catch (e) {
        statusDiv.textContent = `Status: Solver error: ${e}`;
        console.error(e);
    }
}

// Render candidates based on activeCandidates array
function renderCandidates() {
    candidatesList.innerHTML = "";
    
    if (activeSlotId === null) {
        candidatesList.innerHTML = `<div class="no-candidates">Select a word slot to see options.</div>`;
        return;
    }
    
    if (activeCandidates.length === 0) {
        candidatesList.innerHTML = `<div class="no-candidates" style="color: #ef4444;">No viable candidate words match the current board constraints!</div>`;
        return;
    }
    
    activeCandidates.forEach(cand => {
        const item = document.createElement("div");
        item.className = `candidate-item ${cand.state}`;
        item.textContent = cand.word;
        
        item.addEventListener("click", () => {
            fillSlot(activeSlotId, cand.word);
        });
        
        cand.element = item;
        candidatesList.appendChild(item);
    });
}

// Cancel current validation task and rebuild candidate list
function updateActiveCandidates(options) {
    if (currentValidationTask !== null) {
        clearTimeout(currentValidationTask);
        currentValidationTask = null;
    }
    
    activeCandidates = options.map(word => ({ word, state: "pending", element: null }));
    renderCandidates();
    
    if (activeSlotId !== null && activeCandidates.length > 0) {
        startBackgroundValidation(activeSlotId);
    }
}

// Check each pending candidate one-by-one in the background
function startBackgroundValidation(slotId) {
    const fillJson = JSON.stringify(getBoardFill());
    
    function validateNext() {
        if (slotId !== activeSlotId || !solver) {
            currentValidationTask = null;
            return;
        }
        
        // Find the first pending candidate
        const pendingIdx = activeCandidates.findIndex(cand => cand.state === "pending");
        if (pendingIdx === -1) {
            currentValidationTask = null;
            return;
        }
        
        const candidate = activeCandidates[pendingIdx];
        const isFillable = solver.validate_candidate(slotId, candidate.word, fillJson);
        
        if (slotId !== activeSlotId) {
            currentValidationTask = null;
            return;
        }
        
        if (isFillable) {
            candidate.state = "valid";
            if (candidate.element) {
                candidate.element.className = "candidate-item valid";
                // Move element to the top of the container
                candidatesList.insertBefore(candidate.element, candidatesList.firstChild);
            }
            
            // Sort valid ones first in our data array
            activeCandidates.sort((a, b) => {
                if (a.state === "valid" && b.state !== "valid") return -1;
                if (a.state !== "valid" && b.state === "valid") return 1;
                return 0;
            });
        } else {
            // Remove element from DOM
            if (candidate.element) {
                candidate.element.remove();
            }
            // Remove candidate from array
            activeCandidates.splice(pendingIdx, 1);
            
            if (activeCandidates.length === 0) {
                candidatesList.innerHTML = `<div class="no-candidates" style="color: #ef4444;">No viable candidate words match the current board constraints!</div>`;
            }
        }
        
        // Schedule next check on next event loop tick
        currentValidationTask = setTimeout(validateNext, 0);
    }
    
    currentValidationTask = setTimeout(validateNext, 0);
}

// Check if a slot has any empty cells
function isSlotIncomplete(slotId) {
    const slot = slotConfigs[slotId];
    return slot.cells.some(cellName => {
        const inp = document.querySelector(`input[data-slot-id="${slotId}"][data-cell="${cellName}"]`);
        return !inp || inp.value === "";
    });
}

// Find and select the incomplete slot with the fewest remaining options
function selectNextConstrainedSlot() {
    let bestSlotId = null;
    let minOptions = Infinity;
    
    slotConfigs.forEach(slot => {
        if (isSlotIncomplete(slot.id)) {
            const count = remainingOptions[slot.id].length;
            if (count > 0 && count < minOptions) {
                minOptions = count;
                bestSlotId = slot.id;
            }
        }
    });
    
    if (bestSlotId !== null) {
        selectSlot(bestSlotId);
        
        // Focus the first empty input in the newly selected slot row
        const slot = slotConfigs[bestSlotId];
        for (let cellName of slot.cells) {
            const inp = document.querySelector(`input[data-slot-id="${bestSlotId}"][data-cell="${cellName}"]`);
            if (inp && inp.value === "") {
                inp.focus();
                break;
            }
        }
    }
}

// Autofill a slot with a word selection
function fillSlot(slotId, word) {
    const slot = slotConfigs[slotId];
    for (let i = 0; i < slot.cells.length; i++) {
        const cellName = slot.cells[i];
        const letter = word[i];
        document.querySelectorAll(`input[data-cell="${cellName}"]`).forEach(inp => {
            inp.value = letter;
        });
    }
    
    // Propagate constraint changes
    propagateConstraints();
    
    // Jump to the most constrained incomplete slot
    selectNextConstrainedSlot();
}

// Event Listeners
initBtn.addEventListener("click", initializeSolver);

uploadBtn.addEventListener("click", () => {
    dictFileInput.click();
});

dictFileInput.addEventListener("change", (e) => {
    const file = e.target.files[0];
    if (!file) return;
    
    uploadedDictName.textContent = file.name;
    statusDiv.textContent = `Status: Reading ${file.name}...`;
    
    const reader = new FileReader();
    reader.onload = (event) => {
        dictContents = event.target.result;
        initializeSolver();
    };
    reader.onerror = (err) => {
        statusDiv.textContent = `Status: Error reading file: ${err}`;
    };
    reader.readAsText(file);
});

minScoreInput.addEventListener("change", (e) => {
    if (solver) {
        const val = parseInt(e.target.value) || 0;
        solver.set_min_score(val);
        statusDiv.textContent = `Status: Min score updated to ${val}. Re-running constraints...`;
        propagateConstraints();
        statusDiv.textContent = `Status: Min score updated.`;
    }
});

// Start
run();
