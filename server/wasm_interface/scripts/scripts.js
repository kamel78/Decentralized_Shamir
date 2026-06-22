import { dragStart, startResize, copyRowText, bringToFront, user_as_table,
         serialize_wShamir_list, deserialize_wShamir_list,send_share_to_autority,
         broadcast_sub_shares } from './core.js';


export function initializeTablesZIndex() 
    {
        document.querySelectorAll('.table-wrapper').forEach((table, index) => {
            table.style.zIndex = 10 + index; 
        });
    }

export async function loadWasm() {
        const { default: init, WShamirUser, PubKeyAdder, Combiner,EicCrypt } = await import("../pkg/wasm_interface.js");
        await init();
        window.WShamirUser = WShamirUser;
        window.PubKeyAdder = PubKeyAdder;
        window.Combiner = Combiner;
        window.EicCrypt = EicCrypt;
        window.highestZIndex = 10;
        document.dispatchEvent(new Event("wasmLoaded"));
    }
export function save_share_conf() 
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
                global_count: global_count,
                shares_count :shares_count
            };
            localStorage.setItem('serializedHTML', serializedHTML);
            localStorage.setItem('globalState', JSON.stringify(globalState));
        } catch (error) {
            console.error('Error saving state:', error);
        }
    }
export function restore_share_conf() 
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
            if (savedGlobalState) {
                const state = JSON.parse(savedGlobalState);
                if (state.global_count >0) {
                            wShamir_list = deserialize_wShamir_list(state.wShamir_list);
                            users_list = state.users_list;
                            global_threshold = state.global_threshold;
                            global_count = state.global_count;
                            shares_count = state.shares_count;
                            let tables = document.querySelectorAll("table");
                            tables.forEach(table => {                                  
                                    if (table.tBodies[0].rows[0].cells[0].innerHTML.indexOf("Autorité")>-1) 
                                            {   rec_table = table;}
                                });
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
                                btn.onclick = function() { broadcast_sub_shares(btn.getAttribute("username"), btn); };
                                }); 
                            document.getElementById('save-button').disabled = false;   
                            let allSent = Array.from(document.querySelectorAll(".send-btn"))
                                               .every(button => button.classList.contains('sent'));            
                            document.getElementById('rec-button').disabled = !allSent;
                            document.getElementById('enc-button').disabled = false;   
                            document.getElementById('userCount').value = global_count;   
                            document.getElementById('threshold').value = global_threshold;   
                            let copybtn = document.getElementById('copy-btn');   
                            copybtn.addEventListener("click", () => {
                                let pkey =rec_table.tBodies[0].rows[rec_table.tBodies[0].rows.length-1].textContent.split(':').slice(1).join(':').trim();
                                copyRowText(pkey)});
                            console.log('enabled') ;
                            }       
            }
        } catch (error) {
            console.error('Error restoring state:', error);
        }
    }

export function generate_users_conf() 
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
            const newname = 'User ' + (i + 1);
            users_list.push(newname);
            container.appendChild(user_as_table(newname, angle, radius, i));
        }
        users_list.forEach (uname => {
            wShamir_list.set(uname,new WShamirUser(users_list,uname,global_threshold));
            wShamir_list.get(uname).generate_secret();
        })
    }

export function reset_share_conf()
    {
        document.querySelectorAll('.table-wrapper').forEach(wrapper => wrapper.remove());
        document.getElementById('save-button').disabled = true;
        document.getElementById('rec-button').disabled = true;
        document.getElementById('enc-button').disabled = true;
        users_list = [];
        wShamir_list = new Map();
        global_threshold =0;
        global_count =0;
        shares_count = 0;
        save_share_conf();
    }
export function load_shares_for_reconstruction() 
    {
        try {          
            const savedHTML = localStorage.getItem('serializedHTML');
            const container = document.getElementById('tableContainer');
            const savedGlobalState = localStorage.getItem('globalState');
            if (savedGlobalState) {                
                const state = JSON.parse(savedGlobalState);
                if (state.shares_count == 0) {return}
            }
            else {return}
            if (container && savedHTML) {
                container.innerHTML = savedHTML; 
            } else if (!container) {
                console.error("Container #tableContainer not found!");
            }
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
                        let tables = document.querySelectorAll("table");
                        tables.forEach(table => {
                                let num_lines = table.tBodies[0].rows.length;    
                                for (let i = 0; i<num_lines-3; i++) {table.tBodies[0].deleteRow(1)}
                                if (table.tBodies[0].rows[0].cells[0].innerHTML.indexOf("Autorité")>-1) 
                                        {   table.tBodies[0].deleteRow(1);
                                            table.tBodies[0].rows[0].cells[0].textContent = "Autorité de Reconstruction ";
                                            let KeyCell = table.querySelector("tr:last-child td");
                                            KeyCell.childNodes[0].nodeValue = "Clé secrète :    ";
                                            let copyButton = table.querySelector("tr:last-child td button");                                            
                                            copyButton.addEventListener("click", () => {
                                                                            let key = KeyCell.childNodes[0].nodeValue.split(" : ").slice(1).join(" : ");
                                                                            copyRowText(key)}
                                                                       );
                                            auth_table = table;
                                        }
                            });

                        const tableResizer = document.getElementsByClassName('resizer');
                        Array.from(tableResizer).forEach(resizer => {
                            resizer.addEventListener('mousedown', startResize);
                            });
                        const tableBtnsend = document.getElementsByClassName('sent');
                        Array.from(tableBtnsend).forEach(btn => {                                
                            btn.classList.add('send-btn');                    
                            btn.classList.remove('sent');                    
                            btn.textContent ="Envoie";            
                            btn.disabled = false;
                            btn.onclick = function() { send_share_to_autority(btn.getAttribute("username"), btn); };
                            }); 
                        document.getElementById('rec-button').disabled = false;
                        document.getElementById('enc-button').disabled = false;   
                        document.getElementById('userCount').value = global_count;   
                        document.getElementById('threshold').value = global_threshold;                           
                        console.log('enabled') ;
                }
        } catch (error) {
            console.error('Error restoring state:', error);
        }
    }
export function random_reconstruction()
    {
        load_shares_for_reconstruction();
        let keys = Array.from(wShamir_list.keys()); 
        let shuffled = keys.sort(() => Math.random() - 0.5); 
        let selectedKeys = shuffled.slice(0, global_threshold);     
        let selected_users = selectedKeys.map(key => [key, wShamir_list.get(key)]);
        let btns = document.querySelectorAll(".send-btn");
        btns.forEach(btn =>{
                let user = btn.getAttribute("username");
                if (selected_users.find(([key, _]) => key === user)) { send_share_to_autority(user,btn)}
                });
        let selected_shares = selectedKeys.map(key => [key, wShamir_list.get(key).get_share()]);
        let acombiner = new Combiner (users_list,global_threshold);
        let secret_key = acombiner.combine_shares(selected_shares);
        let KeyCell = auth_table.querySelector("tr:last-child td");
        KeyCell.childNodes[0].nodeValue = "Clé secrète : "+secret_key;
        
    }

window.initializeTablesZIndex = initializeTablesZIndex;
window.loadWasm = loadWasm;
window.save_share_conf = save_share_conf;
window.restore_share_conf = restore_share_conf;
window.reset_share_conf = reset_share_conf;
window.generate_users_conf = generate_users_conf;
window.load_shares_for_reconstruction = load_shares_for_reconstruction;
window.random_reconstruction = random_reconstruction;








