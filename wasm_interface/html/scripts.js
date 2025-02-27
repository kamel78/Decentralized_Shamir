// Serialise the array of WShamir Objects and convert to base64 string
function serialize_wShamir_list() 
    {
        let serialized_map = {};
        wShamir_list.forEach((wShamirUser, uname) => {
            serialized_map[uname] = wShamirUser.serialize(); 
        });
        return JSON.stringify(serialized_map, null, 2); 
    }

// DeSerialise the array of WShamir Objects from base64 string
function deserialize_wShamir_list(jsonString) 
    {   
        let parsedObject = JSON.parse(jsonString); 
        let newMap = new Map();
        Object.entries(parsedObject).forEach(([uname, serializedData]) => {
            let wUser = WShamirUser.new_from_serialized(serializedData); 
            newMap.set(uname, wUser);
        });
        return newMap;
    }

// Save page state to be loaded next 

function clearData(){
    document.querySelectorAll('.table-wrapper').forEach(wrapper => wrapper.remove());
    document.getElementById('save-button').disabled = true;
    document.getElementById('rec-button').disabled = true;
    document.getElementById('enc-button').disabled = true;
    users_list = [];
    wShamir_list = new Map();
    global_threshold =0;
    global_count =0;
    saveState();
}

function saveState() 
    {
        try {
            const container = document.getElementById('tableContainer'); 
            if (!container) {
                console.error("Container #tableContainer not found!");
                return;
            }
            const serializedHTML = container.innerHTML;
            let serialized_data = serialize_wShamir_list();
            const globalState = {
                users_list: users_list,
                wShamir_list: serialized_data, 
                global_threshold: global_threshold,
                global_count: global_count
            };
            localStorage.setItem('serializedHTML', serializedHTML);
            localStorage.setItem('globalState', JSON.stringify(globalState));
        } catch (error) {
            console.error('Error saving state:', error);
        }
    }

// Restore page state when reloaded
function restoreState() 
    {
        try {          
            const savedHTML = localStorage.getItem('serializedHTML');
            const container = document.getElementById('tableContainer');
            if (container && savedHTML) {
                container.innerHTML = savedHTML; 
            } else if (!container) {
                console.error("Container #tableContainer not found!");
            }
            const savedGlobalState = localStorage.getItem('globalState');
            console.log(savedGlobalState) ;
            if (savedGlobalState) {
                const state = JSON.parse(savedGlobalState);
                if (state.global_count >0) {
                            wShamir_list = deserialize_wShamir_list(state.wShamir_list);
                            users_list = state.users_list;
                            global_threshold = state.global_threshold;
                            global_count = state.global_count;

                            const tableWrappers = document.getElementsByClassName('table-wrapper');
                            Array.from(tableWrappers).forEach(wrapper => {
                                wrapper.addEventListener('mousedown', dragStart);
                                wrapper.addEventListener("mousedown", function() {
                                            bringToFront(wrapper);
                                        });
                                });
                            const tableResizer = document.getElementsByClassName('resizer');
                            Array.from(tableResizer).forEach(resizer => {
                                resizer.addEventListener('mousedown', startResize);
                                });
                            const tableBtnsend = document.getElementsByClassName('send-btn');
                            Array.from(tableBtnsend).forEach(btn => {
                                btn.onclick = function() { sendSharePart(btn.getAttribute("username"), btn); };
                                }); 
                            document.getElementById('save-button').disabled = false;   
                            document.getElementById('rec-button').disabled = false;
                            document.getElementById('enc-button').disabled = false;   
                            console.log('enabled') ;
                            }       
            }
        } catch (error) {
            console.error('Error restoring state:', error);
        }
    }

// Functions for table visual adjustement and draging 
function dragStart(event) 
    {
        let wrapper = event.currentTarget;
        let shiftX = event.clientX - wrapper.getBoundingClientRect().left;
        let shiftY = event.clientY - wrapper.getBoundingClientRect().top;            
        function moveAt(pageX, pageY) {
            wrapper.style.left = pageX - shiftX + 'px';
            wrapper.style.top = pageY - shiftY + 'px';
        }
        
        function onMouseMove(event) {
            moveAt(event.pageX, event.pageY);
        }
        
        document.addEventListener('mousemove', onMouseMove);            
        function stopDrag() {
            document.removeEventListener('mousemove', onMouseMove);
            document.removeEventListener('mouseup', stopDrag);
        }           
        document.addEventListener('mouseup', stopDrag);
    }

function startResize(event) 
    {
        event.preventDefault();
        let resizer = event.target;
        let table = resizer.previousElementSibling; 
        let wrapper = table.parentElement;
        let startX = event.clientX; 
        let startWidth = table.getBoundingClientRect().width; 
        wrapper.removeEventListener('mousedown', dragStart);
            function onMouseMove(event) {
            let newWidth = event.clientX - table.getBoundingClientRect().left;
            table.style.width = newWidth + "px";
            wrapper.style.width = `${newWidth}px`;

            resizer.style.width = "5px";  
            resizer.style.right = "0";    
            }
            function stopResize() {
                    document.removeEventListener('mousemove', onMouseMove);
                    document.removeEventListener('mouseup', stopResize);
                    wrapper.addEventListener('mousedown', dragStart);
                }
            document.addEventListener('mousemove', onMouseMove);
            document.addEventListener('mouseup', stopResize);
    }
        
function copyRowText(text) 
    {
        navigator.clipboard.writeText(text).then(() => {
            alert("Copied: " + text);
        }).catch(err => {
            console.error("Failed to copy text: ", err);
        });
    }

function bringToFront(element) 
    {
        highestZIndex++;  
        element.style.zIndex = highestZIndex;
    }

function initializeTablesZIndex() 
    {
        document.querySelectorAll('.table-wrapper').forEach((table, index) => {
            table.style.zIndex = 10 + index; 
        });
    }

// generating wShamir structure and user's randmo names
function generate_shamir_users()
    {
        users_list.forEach (uname => {
            wShamir_list.set(uname,new WShamirUser(users_list,uname,global_threshold));
            wShamir_list.get(uname).generate_secret();
        })
    }

function generateRandomUser(index) 
    {
        const newname = 'User ' + (index + 1);
        users_list.push(newname);
        return newname;
    }

// Tables creating and displaying functions 

function generateTable(user, angle, radius, index) 
    {
        let wrapper = document.createElement('div');
        wrapper.classList.add('table-wrapper');

        let x = 40 + radius * Math.cos(angle);
        let y = 40 + radius * Math.sin(angle);
        wrapper.style.top = y + '%';
        wrapper.style.left = x + '%';

        let button = document.createElement('button');
        button.classList.add('send-btn');
        button.textContent = 'Diffuser';
        button.setAttribute("username", user);
        button.onclick = function() { sendSharePart(user, button); };

        let table = document.createElement('table');

        // Create tbody to hold all rows
        let tbody = document.createElement('tbody');

        let headerRow = document.createElement('tr');
        let headerCell = document.createElement('th');
        headerCell.style.width = "300px";
        headerCell.textContent = user;
        headerRow.appendChild(headerCell);
        tbody.appendChild(headerRow);

        let shareRow = document.createElement('tr');
        shareRow.classList.add('fixed-row');
        let shareCell = document.createElement('td');
        shareCell.colSpan = 1;
        shareCell.textContent = 'Partage secret :';
        shareCell.style.backgroundColor = "rgb(230, 175, 9)";
        shareCell.style.color = "black";
        shareCell.style.fontWeight = "bold";
        shareCell.style.fontSize = "13px";
        shareRow.appendChild(shareCell);
        tbody.appendChild(shareRow);

        let publicShareRow = document.createElement('tr');
        publicShareRow.classList.add('fixed-row');
        let publicShareCell = document.createElement('td');
        publicShareCell.colSpan = 1;
        publicShareCell.textContent = 'Partage publique :';
        publicShareCell.style.backgroundColor = "rgb(230, 145, 9)";
        publicShareCell.style.color = "black";
        publicShareCell.style.fontWeight = "bold";
        publicShareCell.style.textAlign = "left";
        publicShareCell.style.fontSize = "13px";
        publicShareRow.appendChild(publicShareCell);
        tbody.appendChild(publicShareRow);

        table.appendChild(tbody); 

        let resizer = document.createElement('div');
        resizer.classList.add('resizer');
        resizer.addEventListener('mousedown', startResize);

        wrapper.appendChild(button);
        wrapper.appendChild(table);
        wrapper.appendChild(resizer);
        wrapper.addEventListener('mousedown', dragStart);
        wrapper.addEventListener("mousedown", function() { bringToFront(wrapper); });
        document.getElementById('save-button').disabled = false;
        document.getElementById('rec-button').disabled = false;
        document.getElementById('enc-button').disabled = false;
        return wrapper;
    }

function drawTables() 
    {
        const container = document.getElementById("tableContainer");
        container.innerHTML = "";
        let userCount = parseInt(document.getElementById("userCount").value) || 3;
        let threshold = parseInt(document.getElementById("threshold").value) || 0;
        users_list = [];
        wShamir_list.clear();
        global_count = userCount;
        global_threshold = threshold;
        let radius = 35;            
        for (let i = 0; i < userCount; i++) {
            let angle = (i / userCount) * (2 * Math.PI);
            container.appendChild(generateTable(generateRandomUser(i), angle, radius, i));
        }
        generate_shamir_users();
    }

// Broadcasting sub-shares for all users 
function sendSharePart(sender, button) 
    {
        if (button.classList.contains('sent')) return;           
        let tables = document.querySelectorAll("table");
        tables.forEach(table => {
            let receiver_name = table.tBodies[0].rows[0].cells[0].innerHTML;
            let share_part = wShamir_list.get(sender).get_secret_part_for_user(receiver_name); 
            let receiver = wShamir_list.get(receiver_name);
            receiver.update_share(sender, share_part);
            let receiver_sec_share = receiver.get_share();
            let receiver_pub_share = receiver.get_partial_pubkey();

            let share_message = `${sender} : <span style="color: blue;">${share_part}</span>`;
            let row = document.createElement("tr");
            let cell = document.createElement("td");
            cell.innerHTML = share_message;
            cell.style.fontSize = "13px";
            row.appendChild(cell);
            let shareRow = table.tBodies[0].querySelector(".fixed-row");
            table.tBodies[0].insertBefore(row, shareRow);
            shareRow.cells[0].textContent = 'Partage secret : ' + receiver_sec_share;
            shareRow.nextElementSibling.cells[0].textContent = 'Partage publique : ' + receiver_pub_share;
        });
        button.classList.add('sent');
        button.textContent = 'Diffusé';
        checkAllSent();
    }

// Check if Broadcasting scheme is completed and construct the final public key
function checkAllSent() 
    {
        let allButtons = document.querySelectorAll(".send-btn");
        let allSent = Array.from(allButtons).every(button => button.classList.contains('sent'));            
        if (allSent) {
            showFinalKey();
        }
    }

// Construct the final public key and display the result 
function showFinalKey() 
    {
        let container = document.getElementById("tableContainer");
        let existingAuthorityTable = document.getElementById("authorityTable");
        if (existingAuthorityTable) {
                existingAuthorityTable.remove();
            }
        let wrapper = document.createElement("div");
        wrapper.id = "authorityTable";
        wrapper.classList.add("table-wrapper");
        wrapper.style.position = "absolute";
        wrapper.style.top = `${window.innerHeight / 2 - 250}px`;
        wrapper.style.left = `${window.innerWidth / 2 - 300}px`; 
        wrapper.style.width = "730px"; 
        wrapper.style.border = "1px solid black";
        wrapper.style.overflow = "hidden";
        wrapper.style.cursor = "grab";  
        let table = document.createElement("table");
        table.style.width = "100%";
        table.style.borderCollapse = "collapse";
        let headerRow = document.createElement("tr");
        let headerCell = document.createElement("th");
        headerCell.textContent = "Autorité de Publication ";
        headerCell.style.backgroundColor = "rgb(165, 19, 19)";
        headerCell.style.padding = "10px";
        headerCell.style.textAlign = "center";
        headerCell.style.color = "white";
        headerRow.appendChild(headerCell);
        table.appendChild(headerRow);
        table.style.zIndex=999;
        let adder = new PubKeyAdder();
        let tables = document.querySelectorAll(".table-wrapper table");
        tables.forEach(tbl => {
            let rows = tbl.querySelectorAll("tr"); 
            let firstRow = rows.length > 1 ? rows[0] : null; 
            let lastRow = rows.length > 2 ? rows[rows.length - 1] : null; 
            if (firstRow && lastRow) {
                let newRow = document.createElement("tr");
                let newCell = document.createElement("td");
                let parts = lastRow.cells[0].innerHTML.split(" : ");
                let pub = parts.slice(1).join(" : ");
                newCell.innerHTML = "Part pub. "+firstRow.cells[0].innerHTML+": "+'<span style="color: blue;">'+pub+'</span>';
                newCell.style.fontSize = "13px";
                newCell.style.padding = "5px";
                newRow.appendChild(newCell);
                table.appendChild(newRow);
                adder.add(pub);
            }
        });
        let fixedRow = document.createElement("tr");
        let fixedCell = document.createElement("td");
        fixedCell.style.fontSize = "18px";
        fixedCell.style.color = "black";
        fixedCell.style.fontWeight = "bold";
        fixedCell.style.textAlign = "left";
        fixedCell.style.padding = "4px";
        fixedCell.style.backgroundColor = "rgb(230, 145, 9)"; 
        fixedRow.appendChild(fixedCell);
        fixedRow.style.height="80px";
        let textNode = document.createTextNode("Clé publique : "+ adder.get_pubkey() + " "); 
        let button = document.createElement("button");
        button.textContent = "📋 Copier"; 
        button.style.marginLeft = "10px"; 
        button.onclick = function () {
                    copyRowText(adder.get_pubkey());
                };
        fixedCell.appendChild(textNode);
        fixedCell.appendChild(button);
        table.appendChild(fixedRow);
        wrapper.appendChild(table);
        let resizer = document.createElement('div');
        resizer.classList.add('resizer');
        resizer.addEventListener('mousedown', startResize);
        wrapper.appendChild(resizer);
        wrapper.addEventListener('mousedown', dragStart);
        wrapper.addEventListener("mousedown", function() {
                    bringToFront(wrapper);
                });
        container.appendChild(wrapper);
        bringToFront(wrapper);
    }

// Save to file the current state
async function saveToFile() {
    try {
        saveState();
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

async function loadWasm() {
    const { default: init, WShamirUser, PubKeyAdder } = await import("../pkg/wasm_interface.js");
    await init();
    window.WShamirUser = WShamirUser;
    window.PubKeyAdder = PubKeyAdder;
    highestZIndex = 10;
    document.dispatchEvent(new Event("wasmLoaded"));
}

// Load from file to the current state
async function loadFromFile() {
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
