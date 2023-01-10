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
    }
)();

startButton.addEventListener('click', () => {
    const code = codeInput.value;

    if (!code) {
        alert("Please enter a code")
    } else {
        wasm.receive(code, codeOutput)
            .then(x => {
                console.log("receiving finished", x);
                if (x) {
                    const {data, filename, filesize} = x;
                    downloadFile(data, filename)
                }
            })
    }
})

fileInput.addEventListener('change', async (e) => {
    const config = wasm.create_config(
        "lothar.com/wormhole/text-or-file-xfer",
        "ws://relay.magic-wormhole.io:4000/v1",
        "",
        2
    )

    console.log(config instanceof wasm.Config)

    debugger;
    const jsConnection = await wasm.create_connection(config);
    debugger;
    wasm.send_file(config, e.target.files[0].name, jsConnection.wormholeAddress);
    /*
    wasm.send(fileInput, codeOutput)
        .then(x => {
            console.log("sending finished");
            })
            */
})


