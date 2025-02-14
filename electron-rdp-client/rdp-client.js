const { spawn } = require('child_process');

class RDPClient {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
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

        // Get the bitmap dimensions
        const width = bitmap.width;
        const height = bitmap.height;

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
        
        const message = {
            type: 'mouse',
            x: Math.floor(x),
            y: Math.floor(y),
            button: event.button,
            is_pressed: true
        };
        console.log('Mouse Down:', message);
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
            scancode: event.keyCode,
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
            scancode: event.keyCode,
            is_pressed: false
        };
        console.log('Key Up:', message);
        this.ws.send(JSON.stringify(message));
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
} 