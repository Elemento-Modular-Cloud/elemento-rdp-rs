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
            // Add frame rate limiting (1 FPS = 1000ms between frames)
            // const now = Date.now();
            // if (now - this.lastFrameTime < 1000) {
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
            dest_left: bitmap.dest_left,
            dest_right: bitmap.dest_right,
            dest_top: bitmap.dest_top,
            dest_bottom: bitmap.dest_bottom,
            bpp: bitmap.bpp,
            dataLength: bitmap.data?.length,
            is_compress: bitmap.is_compress,
            data: bitmap.data
        });

        // Verify that we have valid bitmap data
        if (!bitmap.data || bitmap.data.length === 0) {
            console.error('Invalid or empty bitmap data received');
            return;
        }

        // The bitmap is always 64x64
        const srcWidth = bitmap.width;
        const srcHeight = bitmap.height;
        
        // Calculate the actual update region width and height
        const updateWidth = bitmap.dest_right - bitmap.dest_left;
        const updateHeight = bitmap.dest_bottom - bitmap.dest_top;

        let rgbaData;
        if (bitmap.bpp === 32) {
            // Create array for the bitmap
            rgbaData = new Uint8ClampedArray(srcWidth * srcHeight * 4);
            
            // Copy and convert data from BGRA to RGBA format
            const data = new Uint8Array(bitmap.data);
            for (let i = 0; i < data.length; i += 4) {
                const destIndex = i;
                // Verify source index is within bounds
                if (i + 3 >= data.length) {
                    continue;
                }
                
                // Convert from BGRA to RGBA format
                rgbaData[destIndex] = data[i];     // R (from B)
                rgbaData[destIndex + 1] = data[i + 1]; // G (same position)
                rgbaData[destIndex + 2] = data[i + 2];     // B (from R)
                rgbaData[destIndex + 3] = data[i + 3]; // A (same position)
            }
        } else {
            console.error(`Unsupported bits per pixel: ${bitmap.bpp}`);
            return;
        }

        // Create ImageData for the bitmap
        const imageData = new ImageData(rgbaData, srcWidth, srcHeight);

        // Create and draw the bitmap
        createImageBitmap(imageData).then(imageBitmap => {
            // Clear the destination area first
            this.ctx.clearRect(
                bitmap.dest_left, bitmap.dest_top,
                updateWidth, updateHeight
            );
            
            // Draw the new bitmap
            this.ctx.drawImage(
                imageBitmap,
                /* dx: */ bitmap.dest_left,
                /* dy: */ bitmap.dest_top,
                /* dWidth: */ updateWidth,
                /* dHeight: */ updateHeight
            );
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
        
        this.ws.send(JSON.stringify({
            type: 'scancode',
            scancode: event.keyCode,
            is_pressed: true
        }));
        
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
        
        this.ws.send(JSON.stringify({
            type: 'scancode',
            scancode: event.keyCode,
            is_pressed: false
        }));
    }

    disconnect() {
        if (this.ws) {
            this.ws.close();
        }
        this.connected = false;
    }
} 