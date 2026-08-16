import init, { WasmSolver } from "../pkg/ingrid_core.js";

let solver = null;
let cellNames = [];
let slotConfigs = [];
let activeSlotId = null;
let remainingOptions = [];
let dictContents = "";

// DOM Elements
const initBtn = document.getElementById("init-btn");
const statusDiv = document.getElementById("status");
const slotsInput = document.getElementById("slots-input");
const slotsList = document.getElementById("slots-list");
const cellsBoard = document.getElementById("cells-board");
const candidatesList = document.getElementById("candidates-list");

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
        
        // 2. Configure min score to 50
        solver.set_min_score(50);
        
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
    
    // Render the options list
    renderCandidates();
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
            renderCandidates();
        }
    } catch (e) {
        statusDiv.textContent = `Status: Solver error: ${e}`;
        console.error(e);
    }
}

// Render candidates in the right panel
function renderCandidates() {
    candidatesList.innerHTML = "";
    
    if (activeSlotId === null) {
        candidatesList.innerHTML = `<div class="no-candidates">Select a word slot to see options.</div>`;
        return;
    }
    
    const options = remainingOptions[activeSlotId] || [];
    
    if (options.length === 0) {
        candidatesList.innerHTML = `<div class="no-candidates" style="color: #ef4444;">No viable candidate words match the current board constraints!</div>`;
        return;
    }
    
    options.forEach(word => {
        const item = document.createElement("div");
        item.className = "candidate-item";
        item.textContent = word;
        
        item.addEventListener("click", () => {
            fillSlot(activeSlotId, word);
        });
        
        candidatesList.appendChild(item);
    });
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

// Start
run();
