const { spawn } = require('child_process');

class RDPClient {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        // Create OffscreenCanvas with same dimensions
        this.offscreenCanvas = new OffscreenCanvas(this.canvas.width, this.canvas.height);
        this.offscreenCtx = this.offscreenCanvas.getContext('2d', { alpha: false });
        this.ws = null;
        this.connected = false;
        this.lastFrameTime = 0;  // Add this line to track last frame time
        this.credentials = null;
        this.mstscProcess = null;
        
        // Bind event listeners
        this.canvas.addEventListener('mousedown', this.handleMouseDown.bind(this));
        this.canvas.addEventListener('mouseup', this.handleMouseUp.bind(this));
        this.canvas.addEventListener('mousemove', this.handleMouseMove.bind(this));
        document.addEventListener('keydown', this.handleKeyDown.bind(this));
        document.addEventListener('keyup', this.handleKeyUp.bind(this));
        this.canvas.addEventListener('wheel', this.handleWheel.bind(this));
    }

    connect(credentials) {
        this.credentials = credentials;
        
        // Get the actual canvas display size
        const displayWidth = this.canvas.clientWidth;
        const displayHeight = this.canvas.clientHeight;
        
        // Update canvas internal dimensions to match display size
        this.canvas.width = displayWidth;
        this.canvas.height = displayHeight;
        
        // Launch mstsc-rs process
        try {
            const args = [
                '--target', this.credentials.ip,
                '--user', this.credentials.username,
                '--pass', this.credentials.password,
                '--width', displayWidth.toString(),
                '--height', displayHeight.toString()
            ];

            console.log('Launching mstsc-rs with args:', args);

            this.mstscProcess = spawn('./mstsc-rs', args);

            this.mstscProcess.stdout.on('data', (data) => {
                console.log(`stdout: ${data}`);
            });

            this.mstscProcess.stderr.on('data', (data) => {
                console.error(`stderr: ${data}`);
            });

            this.mstscProcess.on('close', (code) => {
                console.log(`mstsc-rs process exited with code ${code}`);
                this.disconnect();
            });

            // Connect to WebSocket after launching mstsc-rs
            const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            const wsUrl = `${wsProtocol}//127.0.0.1:9000`;
            
            // Add WebSocket connection retry logic
            const maxRetries = 10;
            const retryInterval = 500; // 500ms between retries
            let retryCount = 0;

            const tryConnect = () => {
                this.ws = new WebSocket(wsUrl);
                
                this.ws.onopen = () => {
                    console.log('WebSocket connected');
                    this.connected = true;
                };

                this.ws.onmessage = (event) => {
                    try {
                        const message = JSON.parse(event.data);
                        this.handleBitmap(message);
                    } catch (error) {
                        console.error('Error parsing WebSocket message:', error);
                    }
                };

                this.ws.onerror = () => {
                    if (retryCount < maxRetries) {
                        console.log(`WebSocket connection attempt ${retryCount + 1} failed, retrying...`);
                        retryCount++;
                        setTimeout(tryConnect, retryInterval);
                    } else {
                        console.error('Failed to connect to WebSocket after maximum retries');
                        this.disconnect();
                        alert('Failed to connect to RDP server. Please try again.');
                    }
                };

                this.ws.onclose = () => {
                    console.log('WebSocket disconnected');
                    this.connected = false;
                };
            };

            tryConnect();
        } catch (error) {
            console.error('Failed to start mstsc-rs:', error);
            alert('Failed to start RDP client. Please check if mstsc-rs is installed correctly.');
        }
    }

    handleBitmap(bitmap) {
        // Verify that we have valid bitmap data
        if (!bitmap.buffer || bitmap.buffer.length === 0) {
            console.error('Invalid or empty bitmap data received');
            return;
        }

        const width = bitmap.width;
        const height = bitmap.height;

        // Update offscreen canvas dimensions if needed
        if (this.offscreenCanvas.width !== width || this.offscreenCanvas.height !== height) {
            this.offscreenCanvas.width = width;
            this.offscreenCanvas.height = height;
        }

        // Create ImageData only once and reuse the buffer
        if (!this.imageData || this.imageData.width !== width || this.imageData.height !== height) {
            this.imageData = new ImageData(width, height);
        }
        
        const rgbaData = this.imageData.data;
        const data = bitmap.buffer;
        
        // Use a single loop with direct array access
        let destIndex = 0;
        for (let i = 0; i < data.length; i++) {
            const pixelValue = data[i];
            rgbaData[destIndex] = (pixelValue >> 16) & 255;     // R
            rgbaData[destIndex + 1] = (pixelValue >> 8) & 255;  // G
            rgbaData[destIndex + 2] = pixelValue & 255;         // B
            rgbaData[destIndex + 3] = 255;                      // A
            destIndex += 4;
        }

        // Use OffscreenCanvas for bitmap creation and drawing
        this.offscreenCtx.putImageData(this.imageData, 0, 0);
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        this.ctx.drawImage(this.offscreenCanvas, 0, 0, width, height);
    }

    handleMouseDown(event) {
        if (!this.connected) return;
        
        const rect = this.canvas.getBoundingClientRect();
        const x = event.clientX - rect.left;
        const y = event.clientY - rect.top;
        
        const message = {
            type: 'mouse',
            x: Math.floor(x),
            y: Math.floor(y),
            button: event.button,  // 0 = left, 1 = middle, 2 = right
            is_pressed: true
        };
        this.ws.send(JSON.stringify(message));
    }

    handleMouseUp(event) {
        if (!this.connected) return;
        
        const rect = this.canvas.getBoundingClientRect();
        const x = event.clientX - rect.left;
        const y = event.clientY - rect.top;
        
        const message = {
            type: 'mouse',
            x: Math.floor(x),
            y: Math.floor(y),
            button: event.button,
            is_pressed: false
        };
        console.log('Mouse Up:', message);
        this.ws.send(JSON.stringify(message));
    }

    handleMouseMove(event) {
        if (!this.connected) return;
        
        const rect = this.canvas.getBoundingClientRect();
        const x = event.clientX - rect.left;
        const y = event.clientY - rect.top;
        
        const message = {
            type: 'mouse',
            x: Math.floor(x),
            y: Math.floor(y),
            button: null,
            is_pressed: null
        };
        // Only log every 100th move event to avoid console spam
        if (Math.random() < 0.01) console.log('Mouse Move:', message);
        this.ws.send(JSON.stringify(message));
    }

    handleKeyDown(event) {
        if (!this.connected) return;
        
        const message = {
            type: 'scancode',
            scancode: this.keyCodeToScancode(event.code),
            is_pressed: true
        };
        console.log('Key Down:', message);
        this.ws.send(JSON.stringify(message));
        
        // Prevent default browser actions for certain keys
        if (event.keyCode === 116 || // F5
            (event.ctrlKey && (
                event.keyCode === 82 || // Ctrl+R
                event.keyCode === 87 || // Ctrl+W
                event.keyCode === 78 || // Ctrl+N
                event.keyCode === 76   // Ctrl+L
            ))) {
            event.preventDefault();
        }
    }

    handleKeyUp(event) {
        if (!this.connected) return;
        
        const message = {
            type: 'scancode',
            scancode: this.keyCodeToScancode(event.code),
            is_pressed: false
        };
        console.log('Key Up:', message);
        this.ws.send(JSON.stringify(message));
    }

    handleWheel(event) {
        if (!this.connected) return;
        
        event.preventDefault(); // Prevent page scrolling
        
        // Use Page Up/Down for larger steps (with Shift), Arrow Up/Down for smaller steps
        const scancode = event.shiftKey
            ? (event.deltaY > 0 ? 0xE051 : 0xE049) // Page Down : Page Up
            : (event.deltaY > 0 ? 0xE050 : 0xE048); // Arrow Down : Arrow Up
        
        // Simulate key press
        const keyDownMessage = {
            type: 'scancode',
            scancode: scancode,
            is_pressed: true
        };
        this.ws.send(JSON.stringify(keyDownMessage));
        
        // Simulate key release after a short delay
        setTimeout(() => {
            const keyUpMessage = {
                type: 'scancode',
                scancode: scancode,
                is_pressed: false
            };
            this.ws.send(JSON.stringify(keyUpMessage));
        }, 50); // 50ms delay between press and release
        
        if (Math.random() < 0.01) {
            const keyName = scancode === 0xE051 ? 'PageDown' : 
                           scancode === 0xE049 ? 'PageUp' :
                           scancode === 0xE050 ? 'ArrowDown' : 'ArrowUp';
            console.log('Wheel emulated as:', keyName);
        }
    }

    disconnect() {
        if (this.mstscProcess) {
            this.mstscProcess.kill();
            this.mstscProcess = null;
        }
        
        if (this.ws) {
            this.ws.close();
        }
        this.connected = false;
    }

    keyCodeToScancode(code) {
        const scancodeMap = {
            // Letters
            'KeyQ': 0x0010,
            'KeyW': 0x0011,
            'KeyE': 0x0012,
            'KeyR': 0x0013,
            'KeyT': 0x0014,
            'KeyY': 0x0015,
            'KeyU': 0x0016,
            'KeyI': 0x0017,
            'KeyO': 0x0018,
            'KeyP': 0x0019,
            'KeyA': 0x001E,
            'KeyS': 0x001F,
            'KeyD': 0x0020,
            'KeyF': 0x0021,
            'KeyG': 0x0022,
            'KeyH': 0x0023,
            'KeyJ': 0x0024,
            'KeyK': 0x0025,
            'KeyL': 0x0026,
            'KeyZ': 0x002C,
            'KeyX': 0x002D,
            'KeyC': 0x002E,
            'KeyV': 0x002F,
            'KeyB': 0x0030,
            'KeyN': 0x0031,
            'KeyM': 0x0032,

            // Numbers
            'Digit1': 0x0002,
            'Digit2': 0x0003,
            'Digit3': 0x0004,
            'Digit4': 0x0005,
            'Digit5': 0x0006,
            'Digit6': 0x0007,
            'Digit7': 0x0008,
            'Digit8': 0x0009,
            'Digit9': 0x000A,
            'Digit0': 0x000B,

            // Function keys
            'F1': 0x003B,
            'F2': 0x003C,
            'F3': 0x003D,
            'F4': 0x003E,
            'F5': 0x003F,
            'F6': 0x0040,
            'F7': 0x0041,
            'F8': 0x0042,
            'F9': 0x0043,
            'F10': 0x0044,
            'F11': 0x0057,
            'F12': 0x0058,
            'F13': 0x0064,
            'F14': 0x0065,
            'F15': 0x0066,

            // Special keys
            'MetaLeft': 0xE05B,    // Left Windows key
            'MetaRight': 0xE05C,   // Right Windows key
            'Escape': 0x0001,
            'Minus': 0x000C,
            'Equal': 0x000D,
            'Backspace': 0x000E,
            'Tab': 0x000F,
            'BracketLeft': 0x001A,
            'BracketRight': 0x001B,
            'Enter': 0x001C,
            'ControlLeft': 0x001D,
            'Semicolon': 0x0027,
            'Quote': 0x0028,
            'Backquote': 0x0029,
            'ShiftLeft': 0x002A,
            'Backslash': 0x002B,
            'Comma': 0x0033,
            'Period': 0x0034,
            'Slash': 0x0035,
            'ShiftRight': 0x0036,
            'NumpadMultiply': 0x0037,
            'AltLeft': 0x0038,
            'Space': 0x0039,
            'CapsLock': 0x003A,
            'Pause': 0x0045,
            'ScrollLock': 0x0046,

            // Numpad
            'Numpad7': 0x0047,
            'Numpad8': 0x0048,
            'Numpad9': 0x0049,
            'NumpadSubtract': 0x004A,
            'Numpad4': 0x004B,
            'Numpad5': 0x004C,
            'Numpad6': 0x004D,
            'NumpadAdd': 0x004E,
            'Numpad1': 0x004F,
            'Numpad2': 0x0050,
            'Numpad3': 0x0051,
            'Numpad0': 0x0052,
            'NumpadDecimal': 0x0053,
            'NumpadEnter': 0xE01C,
            'NumpadDivide': 0xE035,

            // Navigation and extended keys
            'ControlRight': 0xE01D,
            'AltRight': 0xE038,
            'NumLock': 0xE045,
            'Home': 0xE047,
            'ArrowUp': 0xE048,
            'PageUp': 0xE049,
            'ArrowLeft': 0xE04B,
            'ArrowRight': 0xE04D,
            'End': 0xE04F,
            'ArrowDown': 0xE050,
            'PageDown': 0xE051,
            'Insert': 0xE052,
            'Delete': 0xE053,
            'ContextMenu': 0xE05D
        };

        return scancodeMap[code] || null;
    }
} 