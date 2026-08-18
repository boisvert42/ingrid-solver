import init, { WasmSolver } from "./pkg/ingrid_core.js";

let solver = null;
let currentValidationTaskId = 0;

self.onmessage = async (e) => {
    const { type, payload } = e.data;
    
    switch (type) {
        case "INIT":
            try {
                currentValidationTaskId++; // Abort any ongoing validation loops
                await init();
                solver = new WasmSolver(payload.slotsDef);
                solver.set_min_score(payload.minScore);
                solver.load_dictionary(payload.dictContents);
                
                const cellNames = JSON.parse(solver.get_cell_names());
                const slotConfigs = JSON.parse(solver.get_slot_configs());
                const initialCellValues = JSON.parse(solver.get_prefills());
                self.postMessage({ type: "INIT_SUCCESS", payload: { cellNames, slotConfigs, initialCellValues } });
            } catch (err) {
                self.postMessage({ type: "ERROR", payload: err.toString() });
            }
            break;
            
        case "UPDATE_MIN_SCORE":
            if (solver) {
                solver.set_min_score(payload.minScore);
            }
            break;
            
        case "RUN_AC3":
            if (!solver) return;
            try {
                const resultsJson = solver.run_ac3(payload.fillJson);
                const remainingOptions = JSON.parse(resultsJson);
                self.postMessage({ type: "AC3_SUCCESS", payload: { remainingOptions } });
            } catch (err) {
                self.postMessage({ type: "ERROR", payload: err.toString() });
            }
            break;
            
        case "START_VALIDATION":
            if (!solver) return;
            currentValidationTaskId++;
            const taskId = currentValidationTaskId;
            
            const { slotId, options, fillJson } = payload;
            let idx = 0;
            
            function validateNext() {
                if (taskId !== currentValidationTaskId) return;
                if (idx >= options.length) return;
                
                const word = options[idx];
                const isFillable = solver.validate_candidate(slotId, word, fillJson);
                
                if (taskId !== currentValidationTaskId) return;
                
                self.postMessage({
                    type: "VALIDATION_RESULT",
                    payload: { word, isFillable, slotId }
                });
                
                idx++;
                setTimeout(validateNext, 0);
            }
            
            validateNext();
            break;
            
        case "STOP_VALIDATION":
            currentValidationTaskId++;
            break;
    }
};
