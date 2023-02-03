import * as wasm from "magic-wormhole-wasm";

const fileInput = document.getElementById("file-input")
const codeInput = document.getElementById("code-input")
const codeOutput = document.getElementById("code-output")
const startButton = document.getElementById("button-start")

function downloadFile(data, fileName) {
    const url = window.URL.createObjectURL(new Blob([new Uint8Array(data)]));
    const a = document.createElement('a');
    a.style.display = 'none';
    a.href = url;
    a.download = fileName;
    document.body.appendChild(a);
    a.click();
    window.URL.revokeObjectURL(url);
}

(function () {
    wasm.init()
})();

// fileInput.addEventListener('change', (e) => {
//     const file = e.target.files[0];
// })

startButton.addEventListener('click', () => {
    const code = codeInput.value;

    if (!code) {
        alert("Please enter a code")
    } else {
        let clientConfig = wasm.ClientConfig.client_init("lothar.com/wormhole/text-or-file-xfer", "wss://mailbox.mw.leastauthority.com/v1", "wss://relay.winden.app/", 2);
        clientConfig.receive(code, codeOutput)
            .then(x => {
                console.log("receiving finished", x);
                if (x) {
                    const {data, filename, filesize} = x;
                    downloadFile(data, filename)
                }
            })
    }
})

fileInput.addEventListener('input', (e) => {
    let clientConfig = wasm.ClientConfig.client_init("lothar.com/wormhole/text-or-file-xfer", "wss://mailbox.mw.leastauthority.com/v1", "wss://relay.winden.app/", 2);
    clientConfig.send(e.target.files[0], codeOutput)
        .then(x => {
            console.log("sending finished");
        })
})


