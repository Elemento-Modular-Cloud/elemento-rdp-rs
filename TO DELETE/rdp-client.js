class RDPClient {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.ws = null;
        this.connected = false;
        this.lastFrameTime = 0;  // Add this line to track last frame time
        
        // Bind event listeners
        this.canvas.addEventListener('mousedown', this.handleMouseDown.bind(this));
        this.canvas.addEventListener('mouseup', this.handleMouseUp.bind(this));
        this.canvas.addEventListener('mousemove', this.handleMouseMove.bind(this));
        document.addEventListener('keydown', this.handleKeyDown.bind(this));
        document.addEventListener('keyup', this.handleKeyUp.bind(this));
    }

    connect() {
        const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${wsProtocol}//127.0.0.1:9000`;  // Updated WebSocket endpoint

        this.ws = new WebSocket(wsUrl);
        
        this.ws.onopen = () => {
            console.log('WebSocket connected');
            this.connected = true;
            // No need to send initial config anymore
        };

        this.ws.onmessage = (event) => {
            // // Add frame rate limiting (1 FPS = 1000ms between frames)
            // const now = Date.now();
            // if (now - this.lastFrameTime < 100) {
            //     return;  // Skip this frame if less than 1 second has passed
            // }
            // this.lastFrameTime = now;

            try {
                const message = JSON.parse(event.data);
                this.handleBitmap(message);
            } catch (error) {
                console.error('Error parsing WebSocket message:', error);
            }
        };

        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
            this.connected = false;
        };

        this.ws.onclose = () => {
            console.log('WebSocket disconnected');
            this.connected = false;
        };
    }

    handleBitmap(bitmap) {
        // Add debug logging
        console.log('Received bitmap:', {
            width: bitmap.width,
            height: bitmap.height,
            dataLength: bitmap.buffer?.length
        });

        // Verify that we have valid bitmap data
        if (!bitmap.buffer || bitmap.buffer.length === 0) {
            console.error('Invalid or empty bitmap data received');
            return;
        }

        // Get the bitmap dimensions
        const width = bitmap.width;
        const height = bitmap.height;

        console.log('Bitmap dimensions:', { width, height });
        console.log('Bitmap buffer length:', bitmap.buffer.length);
        console.log('Bitmap buffer:', bitmap.buffer);

        // Create array for the bitmap (4 channels: R,G,B,A)
        const rgbaData = new Uint8ClampedArray(width * height * 4);
        
        // Convert decimal color values to RGBA
        const data = bitmap.buffer;
        for (let i = 0; i < data.length; i++) {
            const pixelValue = parseInt(data[i]);
            const destIndex = i * 4;
            
            // Extract RGB components from decimal value
            const r = (pixelValue >> 16) & 255;  // Red is in bits 16-23
            const g = (pixelValue >> 8) & 255;   // Green is in bits 8-15
            const b = pixelValue & 255;          // Blue is in bits 0-7
            
            rgbaData[destIndex] = r;     // R
            rgbaData[destIndex + 1] = g; // G
            rgbaData[destIndex + 2] = b; // B
            rgbaData[destIndex + 3] = 255; // A (fully opaque)
        }

        // Create ImageData for the bitmap
        const imageData = new ImageData(rgbaData, width, height);

        // Create and draw the bitmap
        createImageBitmap(imageData).then(imageBitmap => {
            // Clear the entire canvas
            this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
            
            // Draw the new bitmap (full screen)
            this.ctx.drawImage(imageBitmap, 0, 0, width, height);
        }).catch(error => {
            console.error('Error creating image bitmap:', error);
        });
    }

    handleMouseDown(event) {
        if (!this.connected) return;
        
        const rect = this.canvas.getBoundingClientRect();
        const x = event.clientX - rect.left;
        const y = event.clientY - rect.top;
        
        this.ws.send(JSON.stringify({
            type: 'mouse',
            x: Math.floor(x),
            y: Math.floor(y),
            button: event.button,
            is_pressed: true
        }));
    }

    handleMouseUp(event) {
        if (!this.connected) return;
        
        const rect = this.canvas.getBoundingClientRect();
        const x = event.clientX - rect.left;
        const y = event.clientY - rect.top;
        
        this.ws.send(JSON.stringify({
            type: 'mouse',
            x: Math.floor(x),
            y: Math.floor(y),
            button: event.button,
            is_pressed: false
        }));
    }

    handleMouseMove(event) {
        if (!this.connected) return;
        
        const rect = this.canvas.getBoundingClientRect();
        const x = event.clientX - rect.left;
        const y = event.clientY - rect.top;
        
        this.ws.send(JSON.stringify({
            type: 'mouse',
            x: Math.floor(x),
            y: Math.floor(y),
            button: null,
            is_pressed: null
        }));
    }

    handleKeyDown(event) {
        if (!this.connected) return;

        console.log('######## Code:', event.code);
        
        const scancode = this.keyCodeToScancode(event.code);
        if (scancode) {
            this.ws.send(JSON.stringify({
                type: 'scancode',
                scancode: scancode,
                is_pressed: true
            }));
            
            // Prevent default browser actions for certain keys
            if (event.code === 'F5' || 
                (event.ctrlKey && (
                    event.code === 'KeyR' || // Ctrl+R
                    event.code === 'KeyW' || // Ctrl+W
                    event.code === 'KeyN' || // Ctrl+N
                    event.code === 'KeyL'    // Ctrl+L
                ))) {
                event.preventDefault();
            }
        }
    }

    handleKeyUp(event) {
        if (!this.connected) return;
        
        console.log('######## Code:', event.code);
        const scancode = this.keyCodeToScancode(event.code);
        if (scancode) {
            this.ws.send(JSON.stringify({
                type: 'scancode',
                scancode: scancode,
                is_pressed: false
            }));
        }
    }

    // Map JavaScript key codes to RDP scancodes using event.code
    keyCodeToScancode(code) {
        // First convert the event.code to the raw keycode
        const keyCodeMap = {
            'KeyA': 65, 'KeyB': 66, 'KeyC': 67, 'KeyD': 68, 'KeyE': 69,
            'KeyF': 70, 'KeyG': 71, 'KeyH': 72, 'KeyI': 73, 'KeyJ': 74,
            'KeyK': 75, 'KeyL': 76, 'KeyM': 77, 'KeyN': 78, 'KeyO': 79,
            'KeyP': 80, 'KeyQ': 81, 'KeyR': 82, 'KeyS': 83, 'KeyT': 84,
            'KeyU': 85, 'KeyV': 86, 'KeyW': 87, 'KeyX': 88, 'KeyY': 89,
            'KeyZ': 90,
            'Digit1': 49, 'Digit2': 50, 'Digit3': 51, 'Digit4': 52, 'Digit5': 53,
            'Digit6': 54, 'Digit7': 55, 'Digit8': 56, 'Digit9': 57, 'Digit0': 48,
            'Enter': 13, 'Space': 32, 'Tab': 9, 'Escape': 27, 'Backspace': 8,
            'ShiftLeft': 16, 'ShiftRight': 16, 'ControlLeft': 17, 'ControlRight': 17,
            'AltLeft': 18, 'AltRight': 18,
            'CapsLock': 20,
            'F1': 112, 'F2': 113, 'F3': 114, 'F4': 115, 'F5': 116,
            'F6': 117, 'F7': 118, 'F8': 119, 'F9': 120, 'F10': 121,
            'F11': 122, 'F12': 123,
            'ArrowLeft': 37, 'ArrowUp': 38, 'ArrowRight': 39, 'ArrowDown': 40,
            'Home': 36, 'End': 35, 'PageUp': 33, 'PageDown': 34,
            'Insert': 45, 'Delete': 46,
            'Minus': 189, 'Equal': 187, 'BracketLeft': 219, 'BracketRight': 221,
            'Semicolon': 186, 'Quote': 222, 'Backquote': 192, 'Backslash': 220,
            'Comma': 188, 'Period': 190, 'Slash': 191
        };

        // Then convert the keycode to RDP scancode
        const scancodeMap = {
            // Letters
            65: 0x001E, // A
            66: 0x0030, // B
            67: 0x002E, // C
            68: 0x0020, // D
            69: 0x0012, // E
            70: 0x0021, // F
            71: 0x0022, // G
            72: 0x0023, // H
            73: 0x0017, // I
            74: 0x0024, // J
            75: 0x0025, // K
            76: 0x0026, // L
            77: 0x0032, // M
            78: 0x0031, // N
            79: 0x0018, // O
            80: 0x0019, // P
            81: 0x0010, // Q
            82: 0x0013, // R
            83: 0x001F, // S
            84: 0x0014, // T
            85: 0x0016, // U
            86: 0x002F, // V
            87: 0x0011, // W
            88: 0x002D, // X
            89: 0x0015, // Y
            90: 0x002C, // Z

            // Numbers
            49: 0x0002, // 1
            50: 0x0003, // 2
            51: 0x0004, // 3
            52: 0x0005, // 4
            53: 0x0006, // 5
            54: 0x0007, // 6
            55: 0x0008, // 7
            56: 0x0009, // 8
            57: 0x000A, // 9
            48: 0x000B, // 0

            // Special keys
            13: 0x001C,  // Enter
            32: 0x0039,  // Space
            9: 0x000F,   // Tab
            27: 0x0001,  // Escape
            8: 0x000E,   // Backspace
            16: 0x002A,  // Shift (Left)
            17: 0x001D,  // Ctrl (Left)
            18: 0x0038,  // Alt (Left)
            20: 0x003A,  // Caps Lock

            // Function keys
            112: 0x003B, // F1
            113: 0x003C, // F2
            114: 0x003D, // F3
            115: 0x003E, // F4
            116: 0x003F, // F5
            117: 0x0040, // F6
            118: 0x0041, // F7
            119: 0x0042, // F8
            120: 0x0043, // F9
            121: 0x0044, // F10
            122: 0x0057, // F11
            123: 0x0058, // F12

            // Navigation
            37: 0xE04B,  // Left
            38: 0xE048,  // Up
            39: 0xE04D,  // Right
            40: 0xE050,  // Down
            36: 0xE047,  // Home
            35: 0xE04F,  // End
            33: 0xE049,  // Page Up
            34: 0xE051,  // Page Down
            45: 0xE052,  // Insert
            46: 0xE053,  // Delete

            // Punctuation/symbols
            189: 0x000C, // Minus
            187: 0x000D, // Equals
            219: 0x001A, // Left bracket
            221: 0x001B, // Right bracket
            186: 0x0027, // Semicolon
            222: 0x0028, // Quote
            192: 0x0029, // Back quote
            220: 0x002B, // Backslash
            188: 0x0033, // Comma
            190: 0x0034, // Period
            191: 0x0035, // Forward slash
        };

        const keyCode = keyCodeMap[code];
        return keyCode ? scancodeMap[keyCode] : null;
    }

    disconnect() {
        if (this.ws) {
            this.ws.close();
        }
        this.connected = false;
    }
} 