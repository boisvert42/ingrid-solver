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

let cellNames = [];
let slotConfigs = [];
let activeSlotId = null;
let remainingOptions = [];
let dictContents = "";
let activeCandidates = [];
let lastFillTime = 0;

// Initialize background solver Web Worker
const worker = new Worker("solver-worker.js", { type: "module" });

// Handle messages from Solver Worker
worker.onmessage = (e) => {
    const { type, payload } = e.data;
    
    switch (type) {
        case "INIT_SUCCESS":
            cellNames = payload.cellNames;
            parseSlotsConfiguration(slotsInput.value);
            renderBoard();
            renderSlotsList();
            propagateConstraints();
            statusDiv.textContent = "Status: Solver Initialized.";
            break;
            
        case "AC3_SUCCESS":
            remainingOptions = payload.remainingOptions;
            
            // Update slot badges with option counts
            slotConfigs.forEach(slot => {
                const count = remainingOptions[slot.id].length;
                const badge = document.getElementById(`slot-len-badge-${slot.id}`);
                if (badge) {
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
                }
            });
            
            // Update active candidates list
            if (activeSlotId !== null) {
                updateActiveCandidates(remainingOptions[activeSlotId] || []);
            }
            break;
            
        case "VALIDATION_RESULT":
            const { word, isFillable, slotId } = payload;
            if (slotId !== activeSlotId) return;
            handleValidationResult(word, isFillable);
            break;
            
        case "ERROR":
            statusDiv.textContent = `Status: Solver error: ${payload}`;
            console.error(payload);
            break;
    }
};

// Initialize Wasm module by downloading default word list
async function run() {
    try {
        statusDiv.textContent = "Status: Downloading default dictionary...";
        const response = await fetch("../resources/spreadthewordlist.dict");
        if (!response.ok) {
            throw new Error(`Failed to load dictionary: ${response.statusText}`);
        }
        dictContents = await response.text();
        
        statusDiv.textContent = "Status: Ready! Click 'Initialize Solver'.";
        initBtn.disabled = false;
        
        // Auto-initialize the default grid
        initializeSolver();
    } catch (e) {
        statusDiv.textContent = `Status: Error initializing: ${e.message}`;
        console.error(e);
    }
}

// Initialize Solver instance in worker thread
function initializeSolver() {
    if (!dictContents) {
        statusDiv.textContent = "Status: Dictionary not loaded yet.";
        return;
    }
    
    statusDiv.textContent = "Status: Initializing solver in background...";
    const slotsDef = slotsInput.value;
    const minScore = parseInt(minScoreInput.value) || 0;
    
    worker.postMessage({
        type: "INIT",
        payload: { slotsDef, minScore, dictContents }
    });
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
    
    // Update active candidates list (triggers background worker validation)
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

// Run AC-3 constraint propagation in worker thread
function propagateConstraints() {
    const fill = getBoardFill();
    worker.postMessage({
        type: "RUN_AC3",
        payload: { fillJson: JSON.stringify(fill) }
    });
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
        
        item.addEventListener("mousedown", () => {
            fillSlot(activeSlotId, cand.word);
        });
        
        cand.element = item;
        candidatesList.appendChild(item);
    });
}

// Cancel current validation task and rebuild candidate list
function updateActiveCandidates(options) {
    worker.postMessage({ type: "STOP_VALIDATION" });
    
    activeCandidates = options.map((word, idx) => ({ word, state: "pending", element: null, originalIdx: idx }));
    renderCandidates();
    
    if (activeSlotId !== null && activeCandidates.length > 0) {
        worker.postMessage({
            type: "START_VALIDATION",
            payload: {
                slotId: activeSlotId,
                options: options,
                fillJson: JSON.stringify(getBoardFill())
            }
        });
    }
}

// Handle validation results returned by the worker
function handleValidationResult(word, isFillable) {
    const candIdx = activeCandidates.findIndex(cand => cand.word === word);
    if (candIdx === -1) return;
    
    const candidate = activeCandidates[candIdx];
    if (isFillable) {
        candidate.state = "valid";
        if (candidate.element) {
            candidate.element.className = "candidate-item valid";
        }
        
        // Sort valid ones first, maintaining their original score order (originalIdx)
        activeCandidates.sort((a, b) => {
            if (a.state === "valid" && b.state !== "valid") return -1;
            if (a.state !== "valid" && b.state === "valid") return 1;
            return a.originalIdx - b.originalIdx;
        });
        
        // Re-append elements in the new sorted order to rearrange the DOM in-place
        activeCandidates.forEach(cand => {
            if (cand.element) {
                candidatesList.appendChild(cand.element);
            }
        });
    } else {
        // Remove element from DOM
        if (candidate.element) {
            candidate.element.remove();
        }
        // Remove candidate from array
        activeCandidates.splice(candIdx, 1);
        
        if (activeCandidates.length === 0) {
            candidatesList.innerHTML = `<div class="no-candidates" style="color: #ef4444;">No viable candidate words match the current board constraints!</div>`;
        }
    }
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
    const now = Date.now();
    if (now - lastFillTime < 300) {
        return; // Ignore double clicks/taps within 300ms
    }
    lastFillTime = now;

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
    const val = parseInt(e.target.value) || 0;
    worker.postMessage({ type: "UPDATE_MIN_SCORE", payload: { minScore: val } });
    statusDiv.textContent = `Status: Min score updated to ${val}. Re-running constraints...`;
    propagateConstraints();
});

// Start
run();
