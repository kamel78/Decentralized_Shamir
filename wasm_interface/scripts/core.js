// Functions for table visual adjustement and draging 

export function dragStart(event) 
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

export function startResize(event) 
    {
        event.preventDefault();
        let resizer = event.target;
        let table = resizer.previousElementSibling; 
        let wrapper = table.parentElement;
        bringToFront(table);
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

export function copyRowText(text) 
    {
        navigator.clipboard.writeText(text).then(() => {
            alert("Copied: " + text);
        }).catch(err => {
            console.error("Failed to copy text: ", err);
        });
    }

export function bringToFront(element) 
    {
        highestZIndex++;  
        element.style.zIndex = highestZIndex;
    }

// Tables creating and displaying functions 
export function user_as_table(user, angle, radius) 
    {
        let wrapper = document.createElement('div');
        wrapper.classList.add('table-wrapper');
        let x = 40 + radius * Math.cos(angle);
        let y = 40 + radius * Math.sin(angle);
        wrapper.style.top = y + '%';
        wrapper.style.left = x + '%';
        let button = document.createElement('button');
        button.classList.add('send-btn');
        button.textContent = 'Diffuse';
        button.setAttribute("username", user);
        button.onclick = function() { broadcast_sub_shares(user, button); };
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
        publicShareCell.style.textAlign = "center";
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

// Serialise the array of WShamir Objects and convert to base64 string
export function serialize_wShamir_list() 
    {
        let serialized_map = {};
        wShamir_list.forEach((wShamirUser, uname) => {
            serialized_map[uname] = wShamirUser.serialize(); 
        });
        return JSON.stringify(serialized_map, null, 2); 
    }

// DeSerialise the array of WShamir Objects from base64 string
export function deserialize_wShamir_list(jsonString) 
    {   
        let parsedObject = JSON.parse(jsonString); 
        let newMap = new Map();
        Object.entries(parsedObject).forEach(([uname, serializedData]) => {
            let wUser = WShamirUser.new_from_serialized(serializedData); 
            newMap.set(uname, wUser);
        });
        return newMap;
    }

// Broadcasting sub-shares for all users 
export function broadcast_sub_shares(sender, button) 
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
        build_and_display_public_key();
        shares_count = global_threshold;
    }
}

// Construct the final public key and display the result 
function build_and_display_public_key() 
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
    wrapper.style.width = "660px"; 
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
    headerCell.style.textAlign = "left";
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
    button.id ="copy-btn";
    button.innerHTML = '<i class="fa-regular fa-copy"></i>'; 
    button.style.border = "none";
    button.style.background = "none";
    button.style.cursor = "pointer";
    button.style.fontSize = "20px"; 
    button.style.padding = "5px"; 
    button.style.display = "inline-block";
    button.addEventListener("click", () => {copyRowText(adder.get_pubkey());}
    );
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

// Sending send to autority for reconstruction
export function send_share_to_autority(sender, button) 
    {
        let sended_count = auth_table.tBodies[0].rows.length-2;        
        if ((button.classList.contains('sent')) || (sended_count >= global_threshold)) return;           
        let row = auth_table.tBodies[0].insertRow(auth_table.tBodies[0].rows.length-1);
        let cell = document.createElement("td");
        let share_part = wShamir_list.get(sender).get_share();
        let share_message = `${sender} : <span style="color: blue;">${share_part}</span>`;
        cell.innerHTML = share_message;
        cell.style.fontSize = "13px";
        row.appendChild(cell);             
        button.classList.add('sent');
        button.textContent = 'Envoyer';
        if (sended_count == global_threshold-1) {
            let users_selection =[];    
            let num_lines = auth_table.tBodies[0].rows.length;    
            for (let i =1; i < num_lines-1;i++) { let parts = auth_table.tBodies[0].rows[i].textContent.split(':');
                                                  users_selection.push([parts[0].trim(),parts[1].trim()]); }
            let acombiner = new Combiner (users_list,global_threshold);
            let secret_key = acombiner.combine_shares(users_selection);
            let KeyCell = auth_table.querySelector("tr:last-child td");
            KeyCell.childNodes[0].nodeValue = "Clé secrète : "+secret_key;
            }
    }