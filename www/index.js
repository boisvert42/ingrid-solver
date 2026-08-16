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

// Render cells on the board
function renderBoard() {
    cellsBoard.innerHTML = "";
    
    cellNames.forEach(name => {
        const card = document.createElement("div");
        card.className = "cell-card";
        card.dataset.name = name;
        card.id = `cell-${name}`;
        
        const nameLabel = document.createElement("span");
        nameLabel.className = "cell-name";
        nameLabel.textContent = name.toUpperCase();
        
        const input = document.createElement("input");
        input.className = "cell-input";
        input.type = "text";
        input.maxLength = 1;
        input.id = `input-${name}`;
        
        input.addEventListener("input", (e) => {
            e.target.value = e.target.value.toLowerCase().replace(/[^a-z]/g, '');
            propagateConstraints();
        });
        
        card.appendChild(nameLabel);
        card.appendChild(input);
        
        // Focus click behavior
        card.addEventListener("click", () => {
            input.focus();
        });
        
        cellsBoard.appendChild(card);
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
    document.getElementById(`slot-item-${slotId}`).classList.add("active");
    
    // Highlight corresponding cells in the grid
    document.querySelectorAll(".cell-card").forEach(card => {
        card.classList.remove("highlighted");
    });
    
    const activeSlot = slotConfigs[slotId];
    activeSlot.cells.forEach(cellName => {
        document.getElementById(`cell-${cellName}`).classList.add("highlighted");
    });
    
    // Render the options list
    renderCandidates();
}

// Read current board fill state
function getBoardFill() {
    const fill = {};
    cellNames.forEach(name => {
        const val = document.getElementById(`input-${name}`).value;
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

// Autofill a slot with a word selection
function fillSlot(slotId, word) {
    const slot = slotConfigs[slotId];
    for (let i = 0; i < slot.cells.length; i++) {
        const cellName = slot.cells[i];
        const letter = word[i];
        document.getElementById(`input-${cellName}`).value = letter;
    }
    
    // Propagate constraint changes
    propagateConstraints();
}

// Event Listeners
initBtn.addEventListener("click", initializeSolver);

// Start
run();
