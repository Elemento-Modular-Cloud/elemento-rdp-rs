class RDPClient {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.ws = null;
        this.connected = false;
        
        // Bind event listeners
        this.canvas.addEventListener('mousedown', this.handleMouseDown.bind(this));
        this.canvas.addEventListener('mouseup', this.handleMouseUp.bind(this));
        this.canvas.addEventListener('mousemove', this.handleMouseMove.bind(this));
        document.addEventListener('keydown', this.handleKeyDown.bind(this));
        document.addEventListener('keyup', this.handleKeyUp.bind(this));
    }

    connect() {
        const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${wsProtocol}//${window.location.host}/ws`;  // Updated WebSocket endpoint

        this.ws = new WebSocket(wsUrl);
        
        this.ws.onopen = () => {
            console.log('WebSocket connected');
            this.connected = true;
            // No need to send initial config anymore
        };

        this.ws.onmessage = (event) => {
            console.log('Received message:', event);
            try {
                const message = JSON.parse(event.data);
                if (message.type === 'bitmap') {
                    this.handleBitmap(message);
                }
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
        // Create ImageData from the received bitmap data
        const imageData = new ImageData(
            new Uint8ClampedArray(bitmap.data),
            bitmap.width,
            bitmap.height
        );

        // If compressed, decompress the data first
        if (bitmap.isCompress) {
            // Note: Implement decompression if needed
            console.warn('Compressed bitmaps not yet supported');
            return;
        }

        // Draw the bitmap at the specified coordinates
        createImageBitmap(imageData).then(imageBitmap => {
            this.ctx.drawImage(
                imageBitmap,
                bitmap.destLeft,
                bitmap.destTop,
                bitmap.destRight - bitmap.destLeft,
                bitmap.destBottom - bitmap.destTop
            );
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