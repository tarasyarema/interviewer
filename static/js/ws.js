// const URL = "ws://127.0.0.1:1337" 
const URL = "ws://interviewer-ws.onrender.com"

// Init the socker
const socket = new WebSocket(URL);

// TODO: This seems like a really bad idea, but works
let external_change = false;

// Handles if the editor has been loaded
let is_loaded = false;

const editor = ace.edit(
    document.getElementById("editor"),
    {
        mode: "ace/mode/plain_text",
        selectionStyle: "text",
    }
);

// Set event handler for vim keyboard mapping
document.getElementById("vim-mode").addEventListener("change", ({ target: { checked } }) => editor.setKeyboardHandler(checked ? `ace/keyboard/vim` : ''));

// Get the document object so that we can modify the text
const doc = editor.session.getDocument()

// This should be dynamic
const session_id = "id";
// document.getElementById("session_id").innerText = session_id;

const username = random_name();
document.getElementById("username").innerText = username;

socket.onopen = (_) => {
    console.log("[open] Connection established");

    // Send login message
    socket.send(JSON.stringify({
        session: session_id,
        event: "login",
        username: username,
        data: JSON.stringify({ username: username }),
        ts: new Date().getTime()
    }));

    // Send get current value, if any
    socket.send(JSON.stringify({
        session: session_id,
        event: "get_value",
        username: username,
        data: JSON.stringify({
            target: username,
            text: "",
        }),
        ts: new Date().getTime()
    }));

    is_loaded = false;

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
};

socket.onclose = (event) => {
    if (event.wasClean) {
        console.log(`[close] Connection closed cleanly { code: ${event.code}, reason: ${event.reason} }`);
    } else {
        console.error('[close] Connection died', event);
    }
};

socket.onerror = (error) => console.error(error);

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
        case "set_value":
            external_change = true;

            // Get the current cursor
            const { row, column } = editor.getCursorPosition();

            // Update the whole doc value
            doc.setValue(JSON.parse(data).text);

            // Go back to the initial cursor position
            editor.gotoLine(row, column, true);

            // Finally set the editor as loaded
            if (!is_loaded) {
                document.getElementById("loading").innerText = "";
                is_loaded = !is_loaded;
            }

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
