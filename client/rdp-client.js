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