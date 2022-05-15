import * as wasm from "magic-wormhole-wasm";

const fileInput = document.getElementById("file-input")
const codeInput = document.getElementById("code-input")
const codeOutput = document.getElementById("code-output")
const startButton = document.getElementById("button-start")

startButton.addEventListener('click', () => {
    const code = codeInput.value;

    if (!code) {
        alert("Please enter a code")
    } else {
        wasm.receive(code, codeOutput)
            .then(x => {
                console.log("receiving finished");
            })
    }
})

fileInput.addEventListener('input', () => {
    wasm.send(fileInput, codeOutput)
        .then(x => {
            console.log("sending finished");
        })
})


