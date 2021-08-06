// const URL = "ws://127.0.0.1:1337" 
const URL = "wss://interviewer-ws.onrender.com"

const editor = ace.edit(
    document.getElementById("editor"),
    {
        mode: "ace/mode/plain_text",
        selectionStyle: "text",
    }
);

// Set event handler for vim keyboard mapping
document.getElementById("vim-mode").addEventListener("change", ({ target: { checked } }) => editor.setKeyboardHandler(checked ? `ace/keyboard/vim` : ''));

const urlSearchParams = new URLSearchParams(window.location.search);

const params = Object.fromEntries(urlSearchParams.entries());

let session_id;

if (params.session_id === undefined) {
    params.session_id = new Date().getTime();
    window.location.replace(`/?${new URLSearchParams(params).toString()}`);    
} else {
    session_id = params.session_id;
}

document.getElementById("session_id").innerHTML = `<b>${session_id}</b>`;

// Get the document object so that we can modify the text
const doc = editor.session.getDocument()

let username = random_name();

if (params.username === undefined) {
    username = window.prompt(`Pick your username `, username);
} else {
    username = params.username;
}

document.getElementById("username").innerText = username;

const connect = () => {
    // Init the socker
    const socket = new WebSocket(URL);

    // TODO: This seems like a really bad idea, but works
    let external_change = false;

    socket.onopen = (_) => {
        console.log("[open] Connection established");

        // Send login message
        socket.send(JSON.stringify({
            session: session_id,
            event: "login",
            username: username,
            data: JSON.stringify({ username: username, session_id: session_id }),
            ts: new Date().getTime()
        }));

        editor.on("change", (e) => {
            if (external_change) {
                external_change = !external_change;
                return false;
            }

            socket.send(JSON.stringify({
                session: session_id,
                username: username,
                event: "change",
                data: JSON.stringify(e),
                ts: new Date().getTime()
            }));
        });

        editor.on("changeSelectionStyle", (e) => console.log(e));
    };

    socket.onclose = (event) => {
        if (event.wasClean) {
            console.log(`[close] Connection closed cleanly { code: ${event.code}, reason: ${event.reason} }`);
        } else {
            console.error('[close] Connection died, reconnecting...', event);

            setTimeout(function() {
                connect();
            }, 1000);
        }
    };

    socket.onerror = (error) => {
        console.error(error);
        socket.close();
    }

    // Fun stuff
    socket.onmessage = (e) => {
        const event_data = JSON.parse(e.data);
        const { event, data } = event_data;

        console.log(`[${event}]`, data);

        switch (event) {
            case "change":
                external_change = true;

                doc.applyDelta(JSON.parse(data));
                break;
            case "add_user":
                const li = document.createElement("li");
                li.innerText = JSON.parse(data).username;

                document.getElementById("user_list").appendChild(li);
                break;
            case "remove_user":
                const user_list = document.getElementById("user_list");

                // Copy the list of users so that we do not have strange things
                const lis = [];
                for (const li of user_list.getElementsByTagName("li")) lis.push(li);

                const to_remove = JSON.parse(data).username;

                // Remove the whole list
                document.getElementById("user_list").innerHTML = "";

                for (const user of lis) {
                    // Only append if the current user is not the one to remove
                    if (user.innerText !== to_remove) document.getElementById("user_list").appendChild(user)
                }
                break;
            case "set_value":
                external_change = true;

                // Get the current cursor
                const { row, column } = editor.getCursorPosition();

                // Update the whole doc value
                doc.setValue(JSON.parse(data).text);

                // Go back to the initial cursor position
                editor.gotoLine(row, column, true);

                break;
            case "send_value":
                const value = JSON.stringify({
                    session: session_id,
                    username: username,
                    event: "set_value",
                    data: JSON.stringify({
                        target: event_data.username,
                        text: doc.getValue(),
                    }),
                    ts: new Date().getTime()
                })

                socket.send(value);
                console.log(`[${event}] sent ${value.length} bytes to ${event_data.username}`, value);

                break;
        }
    };
}

connect();
