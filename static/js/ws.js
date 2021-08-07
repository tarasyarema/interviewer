// const URL = "ws://127.0.0.1:1337" 
const URL = "wss://interviewer-ws.onrender.com"

const editor = ace.edit(
    document.getElementById("editor"),
    {
        // TODO: Add list to change it
        mode: "ace/mode/plain_text",
        selectionStyle: "text",
    }
);

// TODO: Add list to be able to change it
editor.setTheme('ace/theme/solarized_light');

document.getElementById('editor').style.fontSize = '14px';
editor.setHighlightActiveLine(true);

// Set event handler for vim keyboard mapping
document.getElementById("vim-mode").addEventListener("change", ({ target: { checked } }) => editor.setKeyboardHandler(checked ? `ace/keyboard/vim` : ''));

const urlSearchParams = new URLSearchParams(window.location.search);
const params = Object.fromEntries(urlSearchParams.entries());

// Global session ID
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

// Current username
let username = random_name();

if (params.username === undefined) {
    username = window.prompt(`Pick your username`, username);
} else {
    username = params.username;
}

// Here we will store the users
let other_users = {};

/**
  * Waits for the WS connection to be in an open state.
  * @param socket The WebSocket connection
  */
const waitForOpenConnection = (socket) => {
    return new Promise((resolve, reject) => {
        const intervalTime = 200; // In ms
        const maxNumberOfAttempts = 10;
        let currentAttempt = 0;

        const interval = setInterval(() => {
            if (currentAttempt > maxNumberOfAttempts - 1) {
                clearInterval(interval);
                reject(new Error('[error] Maximum number of attempts exceeded to reconnect'));
            } else if (socket.readyState === socket.OPEN) {
                clearInterval(interval);
                resolve();
            }
            currentAttempt++;
        }, intervalTime);
    })
}

/**
  * Sends a message only when the socket connection state is open.
  * @param socket The WebSocket connection
  * @param msg The message to send
  */
const sendMessage = async (socket, msg) => {
    if (socket.readyState !== socket.OPEN) {
        try {
            await waitForOpenConnection(socket);
            socket.send(msg);
        } catch (err) { 
            console.error(err);
        }
    } else {
        socket.send(msg);
    }
}

/**
 * Function that handles the whole WS editor logic.
 */
const connect = () => {
    // Reset the document text
    doc.setValue("");

    // Reset the list of users
    document.getElementById("username").innerText = username;
    document.getElementById("user_list").innerHTML = "";

    // Init the socker
    const socket = new WebSocket(URL);

    // TODO: This seems like a really bad idea, but works
    let external_change = false;

    socket.onopen = (open_event) => {
        console.log("[open] Connection established", open_event);

        // Send login message
        sendMessage(socket, JSON.stringify({
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

            sendMessage(socket, JSON.stringify({
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
            console.log('[close] Connection died, reconnecting...', event);

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

                const delta = JSON.parse(data);
                doc.applyDelta(delta);

                // Show which user changed what
                if ('end' in delta) {
                    const changer = event_data.username;

                    // This could be handled as a cursor change
                    // const { row, column } = delta.end;
                    // console.log({ changer, row, column });

                    const el = document.getElementById(`__user_${changer}`);

                    // Solarized light orange
                    el.style.color = '#cb4b16';

                    // Clear the color change after 1_000 ms, i.e. 1 second
                    setTimeout(() => {
                        el.style.color = '';
                    }, 1_000);
                }

                break;
            case "add_user":
                const li = document.createElement("li");

                const target = JSON.parse(data).username;
                other_users[target] = true;

                li.innerText = target;
                li.id = `__user_${target}`;

                document.getElementById("user_list").appendChild(li);
                break;
            case "remove_user":
                const to_remove = JSON.parse(data).username;
                delete other_users[to_remove];

                // Remove the user from the list
                document.getElementById(`__user_${to_remove}`).outerHTML = "";
                break;
            case "set_value":
                const text = JSON.parse(data).text;

                // Mark that the next change event is external
                // and not intentional, but only when it's not empty
                // as when empty the change event will not be triggered
                // on the setValue call
                external_change = text.length > 0 ? true : false;

                // Get the current cursor
                const { row, column } = editor.getCursorPosition();

                // Update the whole doc value
                doc.setValue(text);

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

                sendMessage(socket, value);
                console.log(`[${event}] sent ${value.length} bytes to ${event_data.username}`, value);

                break;
        }
    };
}

connect();
