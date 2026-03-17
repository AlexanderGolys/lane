"use strict";

function main() {
    const canvas = document.querySelector("#canvas");
    if (!canvas) {
        const newCanvas = document.createElement("canvas");
        newCanvas.id = "canvas";
        newCanvas.style.width = "100%";
        newCanvas.style.height = "100%";
        newCanvas.width = window.innerWidth;
        newCanvas.height = window.innerHeight;
        document.body.style.margin = "0";
        document.body.style.overflow = "hidden";
        document.body.appendChild(newCanvas);
        return main();
    }

    const gl = canvas.getContext("webgl2");
    if (!gl) {
        console.error("WebGL2 not supported");
        return;
    }

    const vertexShaderSource = `#version 300 es
        in vec2 a_position;
        out vec2 uv;
        void main() {
            uv = a_position * 0.5 + 0.5;
            gl_Position = vec4(a_position, 0.0, 1.0);
        }
    `;

    const fragmentShaderSource = `#version 300 es
        precision highp float;

        struct paramBall2D {
            float r;
            vec2 center;
        };

        uniform paramBall2D ball1;
        uniform paramBall2D ball2;
        uniform vec2 resolution;
            
        in vec2 uv;
        out vec4 fragColor;


        float sdf_ball2D(vec2 x, paramBall2D p){
          return length(p.center - x) - p.r;
        }

        float unionSDF(float d1, float d2){
          return min(d1, d2);
        }

        float sdf1(vec2 x){
          return sdf_ball2D(x, ball1);
        }

        float sdf2(vec2 x){
          return sdf_ball2D(x, ball2);
        }

        float sdf(vec2 x){
          return unionSDF(sdf1(x), sdf2(x));
        }


        void main() {
            vec4 c1 = vec4(0., 0.8, 1., 1.);
            vec4 c2 = vec4(0.8, 0.8, 0., 1.);
            float d = sdf(uv);
            vec4 c = d > 0. ? c1 : c2;
            fragColor = c*(.5+sin(d*10.)/2.);
        }
    `;

    function createShader(gl, type, source) {
        const shader = gl.createShader(type);
        gl.shaderSource(shader, source);
        gl.compileShader(shader);
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
            console.error(gl.getShaderInfoLog(shader));
            gl.deleteShader(shader);
            return null;
        }
        return shader;
    }

    function createProgram(gl, vertexShader, fragmentShader) {
        const program = gl.createProgram();
        gl.attachShader(program, vertexShader);
        gl.attachShader(program, fragmentShader);
        gl.linkProgram(program);
        if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
            console.error(gl.getProgramInfoLog(program));
            gl.deleteProgram(program);
            return null;
        }
        return program;
    }

    const vertexShader = createShader(gl, gl.VERTEX_SHADER, vertexShaderSource);
    const fragmentShader = createShader(gl, gl.FRAGMENT_SHADER, fragmentShaderSource);
    const program = createProgram(gl, vertexShader, fragmentShader);

    if (!program) {
        console.error("Failed to create program");
        return;
    }

    const positionLocation = gl.getAttribLocation(program, "a_position");
    const ball1Location = gl.getUniformLocation(program, "ball1");
    const ball2Location = gl.getUniformLocation(program, "ball2");
    const resolutionLocation = gl.getUniformLocation(program, "resolution");

    const positionBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);

    const positions = [
        -1, -1,
         3, -1,
        -1,  3,
    ];
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(positions), gl.STATIC_DRAW);

    function resizeCanvasToDisplaySize(canvas) {
        const displayWidth = canvas.clientWidth;
        const displayHeight = canvas.clientHeight;
        if (canvas.width !== displayWidth || canvas.height !== displayHeight) {
            canvas.width = displayWidth;
            canvas.height = displayHeight;
        }
    }

    function drawScene() {
        resizeCanvasToDisplaySize(gl.canvas);

        gl.viewport(0, 0, gl.canvas.width, gl.canvas.height);
        gl.clearColor(0, 0, 0, 1);
        gl.clear(gl.COLOR_BUFFER_BIT);

        gl.useProgram(program);

        gl.enableVertexAttribArray(positionLocation);
        gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
        gl.vertexAttribPointer(positionLocation, 2, gl.FLOAT, false, 0, 0);

        gl.uniform1f(gl.getUniformLocation(program, "ball1.r"), 0.15);
        gl.uniform2f(gl.getUniformLocation(program, "ball1.center"), 0.3, 0.5);
        gl.uniform1f(gl.getUniformLocation(program, "ball2.r"), 0.2);
        gl.uniform2f(gl.getUniformLocation(program, "ball2.center"), 0.7, 0.5);
        gl.uniform2f(resolutionLocation, gl.canvas.width, gl.canvas.height);

        gl.drawArrays(gl.TRIANGLES, 0, 3);

        requestAnimationFrame(drawScene);
    }

    window.addEventListener("resize", () => {
        canvas.width = window.innerWidth;
        canvas.height = window.innerHeight;
    });

    drawScene();
}

main();
