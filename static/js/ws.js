const URL = "ws://127.0.0.1:1337" 
// const URL = "wss://interviewer-ws.onrender.com"

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
    window.location.replace(`/?session_id=${new Date().getTime()}`);    
} else {
    session_id = params.session_id;
}

document.getElementById("session_id").innerHTML = `Session ID: <b>${session_id}</b>`;

// Get the document object so that we can modify the text
const doc = editor.session.getDocument()


const username = random_name();
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
            data: JSON.stringify({ username: username }),
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
