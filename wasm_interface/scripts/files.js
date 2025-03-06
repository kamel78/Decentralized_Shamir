// Save to file the current state
export async function save_conf_ToFile() {
    try {
        save_share_conf();
        const handle = await window.showSaveFilePicker({
            suggestedName: "saved_data.json", 
            types: [
                {
                    description: "JSON Files",
                    accept: { "application/json": [".json"] },
                },  
            ],
        });
        let item1 = localStorage.getItem('globalState') || "";
        let item2 = localStorage.getItem('serializedHTML') || "";
        let data = JSON.stringify({ globalState: item1, serializedHTML: item2 }, null, 2);
        const writable = await handle.createWritable();
        await writable.write(data);
        await writable.close();

        console.log("Fichier enregistrer avec succès !");
    } catch (err) {
        console.error("Enregistrement annulé ou échoué :", err);
    }
}



// Load from file to the current state
export async function load_conf_FromFile() {
    try {
        const [fileHandle] = await window.showOpenFilePicker({
            types: [
                {
                    description: "JSON Files",
                    accept: { "application/json": [".json"] },
                },
            ],
            multiple: false, 
        });
        const file = await fileHandle.getFile();
        const content = await file.text();
        const data = JSON.parse(content);
        localStorage.setItem('globalState', data.globalState);
        localStorage.setItem('serializedHTML', data.serializedHTML);
        loadWasm();
        console.log("Fichier chargé avec succès !");
    } catch (err) {
        console.error("Chargement annulé ou échoué :", err);
    }
}

window.save_conf_ToFile = save_conf_ToFile;
window.load_conf_FromFile = load_conf_FromFile;