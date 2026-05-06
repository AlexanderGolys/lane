"use strict";

const vertexShaderSource = "#version 300 es\nprecision highp float;\n\nconst vec2 vertices[3] = vec2[3](\n    vec2(-1.0, -1.0),\n    vec2(3.0, -1.0),\n    vec2(-1.0, 3.0)\n);\n\nvoid main() {\n    gl_Position = vec4(vertices[gl_VertexID], 0.0, 1.0);\n}\n";

// Paste generated fragment shader GLSL here.
const fragmentShaderSource = "#version 300 es\nprecision highp float;\n\nout vec4 outColor;\n\nuniform vec3 cameraPosition;\nuniform vec3 cameraForward;\nuniform vec3 cameraGlobalUp;\nuniform vec2 resolution;\nuniform vec3 ambientColor;\nuniform float time;\nstruct ParamBall3D {\n    float r;\n};\n\nfloat sdf0_Ball3D(vec3 p, ParamBall3D params) {\n    return length(p) - params.r;\n}\n\nstruct ParamPlane3D {\n    vec3 n;\n    float h;\n};\n\nfloat sdf0_Plane3D(vec3 p, ParamPlane3D params) {\n    return dot(normalize(params.n), p) + params.h;\n}\n\nstruct Camera {\n    vec3 position;\n    vec3 forward;\n    vec3 global_up;\n    vec2 resolution;\n};\n\nstruct Hit {\n    vec3 position;\n    vec3 normal;\n    float travel;\n    bool hit;\n};\n\nstruct Material {\n    vec3 color;\n    vec3 emission;\n    float reflectiveness;\n};\n\nMaterial add_Material(Material a, Material b) {\n    return Material((a.color + b.color), (a.emission + b.emission), (a.reflectiveness + b.reflectiveness));\n}\n\nMaterial scale_Material(Material value, float scalar) {\n    return Material((value.color * scalar), (value.emission * scalar), (value.reflectiveness * scalar));\n}\n\nMaterial sub_Material(Material a, Material b) {\n    return Material((a.color - b.color), (a.emission - b.emission), (a.reflectiveness - b.reflectiveness));\n}\n\nMaterial zero_Material = Material(vec3(0.0f), vec3(0.0f), 0.0f);\n\nstruct Ray {\n    vec3 origin;\n    vec3 dir;\n};\n\nRay add_Ray(Ray a, Ray b) {\n    return Ray((a.origin + b.origin), (a.dir + b.dir));\n}\n\nRay scale_Ray(Ray value, float scalar) {\n    return Ray((value.origin * scalar), (value.dir * scalar));\n}\n\nRay sub_Ray(Ray a, Ray b) {\n    return Ray((a.origin - b.origin), (a.dir - b.dir));\n}\n\nRay zero_Ray = Ray(vec3(0.0f), vec3(0.0f));\n\nstruct RaycolorConfig {\n    int max_bounces;\n    float ray_bias;\n    float throughput_threshold;\n};\n\nstruct RaytraceConfig {\n    int max_steps;\n    float hit_threshold;\n    float max_travel;\n};\n\nmat3 _op_rot_matrix(vec3 _binormal, float _angle) {\n    vec3 _axis = normalize(_binormal);\n    float _c = cos(_angle);\n    float _s = sin(_angle);\n    float _oc = 1.0f - _c;\n    return mat3(\n        vec3((_axis.x * _axis.x * _oc) + _c, (_axis.y * _axis.x * _oc) + (_axis.z * _s), (_axis.z * _axis.x * _oc) - (_axis.y * _s)),\n        vec3((_axis.x * _axis.y * _oc) - (_axis.z * _s), (_axis.y * _axis.y * _oc) + _c, (_axis.z * _axis.y * _oc) + (_axis.x * _s)),\n        vec3((_axis.x * _axis.z * _oc) + (_axis.y * _s), (_axis.y * _axis.z * _oc) - (_axis.x * _s), (_axis.z * _axis.z * _oc) + _c)\n    );\n}\n\nvec3 _op_rot_inverse_point(vec3 _p, vec3 _binormal, vec3 _anchor, float _angle) {\n    mat3 _r = _op_rot_matrix(_binormal, _angle);\n    return _anchor + (transpose(_r) * (_p - _anchor));\n}\n\nfloat _op_smooth_union(float _a, float _b, float _k) {\n    _k *= 1.0f / (1.0f - sqrt(0.5f));\n    float _h = max(_k - abs(_a - _b), 0.0f) / _k;\n    return min(_a, _b) - (_k * 0.5f * (1.0f + _h - sqrt(1.0f - (_h * (_h - 2.0f)))));\n}\n\nfloat _op_union(float _a, float _b) {\n    return min(_a, _b);\n}\n\nMaterial conditional_material(bool condition, Material then_value, Material else_value) {\n    if (condition) {\n        return then_value;\n    }\n    return else_value;\n}\n\nconst RaytraceConfig default_raytrace_config = RaytraceConfig(128, 0.0005f, 100.0f);\nconst RaycolorConfig default_raycolor_config = RaycolorConfig(6, 0.001f, 0.001f);\nconst Material ceiling_light_material = Material(vec3(1.0f, 0.92f, 0.78f), vec3(1.6f, 1.35f, 1.05f), 0.0f);\nconst Material solid_material = Material(vec3(0.72f, 0.42f, 0.28f), vec3(0.0f, 0.0f, 0.0f), 0.32f);\nconst Material ground_material = Material(vec3(0.45f, 0.5f, 0.54f), vec3(0.0f, 0.0f, 0.0f), 0.12f);\n\nfloat sdf_ceiling_light(vec3 p);\nvec3 grad_sdf_ceiling_light(vec3 p);\nfloat sdf_ground(vec3 p);\nvec3 grad_sdf_ground(vec3 p);\nfloat sdf_scene(vec3 p);\nvec3 grad_sdf_scene(vec3 p);\nfloat sdf_solid(vec3 p);\nvec3 grad_sdf_solid(vec3 p);\nfloat sdf_subject(vec3 p);\nvec3 grad_sdf_subject(vec3 p);\n\nvec3 scene_material_color(Hit _t);\nvec3 scene_material_emission(Hit _t);\nfloat scene_material_reflectiveness(Hit _t);\nvec4 scene_shade(vec2 _t);\n\nvec3 hit_position(Hit _t) {\n    Hit _hit = _t;\n    return (_hit).position;\n}\n\nvec3 material_color(Material _t) {\n    Material _material = _t;\n    return (_material).color;\n}\n\nvec3 material_emission(Material _t) {\n    Material _material = _t;\n    return (_material).emission;\n}\n\nfloat material_reflectiveness(Material _t) {\n    Material _material = _t;\n    return (_material).reflectiveness;\n}\n\nvec2 camera_uv(Camera _t0, vec2 _t1) {\n    Camera _camera = _t0;\n    vec2 _v = _t1;\n    return (((_v * 2.0f) - (_camera).resolution) / min(((_camera).resolution).x, ((_camera).resolution).y));\n}\n\nvec3 camera_right(Camera _t) {\n    Camera _camera = _t;\n    return normalize(cross((_camera).forward, (_camera).global_up));\n}\n\nvec3 camera_up(Camera _t) {\n    Camera _camera = _t;\n    return normalize(cross(cross((_camera).forward, (_camera).global_up), (_camera).forward));\n}\n\nfloat sdf_subject_base(vec3 p) {\n    return _op_smooth_union(sdf0_Ball3D((p - vec3((-0.38f), (-0.08f), 3.1f)), ParamBall3D(0.72f)), sdf0_Ball3D((p - vec3(0.42f, 0.14f, 2.7f)), ParamBall3D(0.46f)), 0.18f);\n}\n\nvec3 grad_sdf_subject_base(vec3 p) {\n    float eps = 0.01f;\n    return normalize(vec3(((sdf_subject_base(p + vec3(eps, 0.0f, 0.0f)) - sdf_subject_base(p - vec3(eps, 0.0f, 0.0f))) / (2.0f * eps)), ((sdf_subject_base(p + vec3(0.0f, eps, 0.0f)) - sdf_subject_base(p - vec3(0.0f, eps, 0.0f))) / (2.0f * eps)), ((sdf_subject_base(p + vec3(0.0f, 0.0f, eps)) - sdf_subject_base(p - vec3(0.0f, 0.0f, eps))) / (2.0f * eps))));\n}\n\nfloat sdf_subject(vec3 p) {\n    return sdf_subject_base(_op_rot_inverse_point(p, vec3(0.0f, 1.0f, 0.0f), vec3(0.0f, 0.0f, 2.9f), (time * 0.8f)));\n}\n\nvec3 grad_sdf_subject(vec3 p) {\n    float eps = 0.01f;\n    return normalize(vec3(((sdf_subject(p + vec3(eps, 0.0f, 0.0f)) - sdf_subject(p - vec3(eps, 0.0f, 0.0f))) / (2.0f * eps)), ((sdf_subject(p + vec3(0.0f, eps, 0.0f)) - sdf_subject(p - vec3(0.0f, eps, 0.0f))) / (2.0f * eps)), ((sdf_subject(p + vec3(0.0f, 0.0f, eps)) - sdf_subject(p - vec3(0.0f, 0.0f, eps))) / (2.0f * eps))));\n}\n\nfloat sdf_ground(vec3 p) {\n    return sdf0_Plane3D(p, ParamPlane3D(vec3(0.0f, 1.0f, 0.0f), (-dot(normalize(vec3(0.0f, 1.0f, 0.0f)), vec3(0.0f, (-0.9f), 0.0f)))));\n}\n\nvec3 grad_sdf_ground(vec3 p) {\n    float eps = 0.01f;\n    return normalize(vec3(((sdf_ground(p + vec3(eps, 0.0f, 0.0f)) - sdf_ground(p - vec3(eps, 0.0f, 0.0f))) / (2.0f * eps)), ((sdf_ground(p + vec3(0.0f, eps, 0.0f)) - sdf_ground(p - vec3(0.0f, eps, 0.0f))) / (2.0f * eps)), ((sdf_ground(p + vec3(0.0f, 0.0f, eps)) - sdf_ground(p - vec3(0.0f, 0.0f, eps))) / (2.0f * eps))));\n}\n\nfloat sdf_ceiling_light(vec3 p) {\n    return sdf0_Plane3D(p, ParamPlane3D(vec3(0.0f, (-1.0f), 0.0f), (-dot(normalize(vec3(0.0f, (-1.0f), 0.0f)), vec3(0.0f, 70.0f, 0.0f)))));\n}\n\nvec3 grad_sdf_ceiling_light(vec3 p) {\n    float eps = 0.01f;\n    return normalize(vec3(((sdf_ceiling_light(p + vec3(eps, 0.0f, 0.0f)) - sdf_ceiling_light(p - vec3(eps, 0.0f, 0.0f))) / (2.0f * eps)), ((sdf_ceiling_light(p + vec3(0.0f, eps, 0.0f)) - sdf_ceiling_light(p - vec3(0.0f, eps, 0.0f))) / (2.0f * eps)), ((sdf_ceiling_light(p + vec3(0.0f, 0.0f, eps)) - sdf_ceiling_light(p - vec3(0.0f, 0.0f, eps))) / (2.0f * eps))));\n}\n\nfloat sdf_solid(vec3 p) {\n    return _op_union(sdf_subject(p), sdf_ground(p));\n}\n\nvec3 grad_sdf_solid(vec3 p) {\n    float eps = 0.01f;\n    return normalize(vec3(((sdf_solid(p + vec3(eps, 0.0f, 0.0f)) - sdf_solid(p - vec3(eps, 0.0f, 0.0f))) / (2.0f * eps)), ((sdf_solid(p + vec3(0.0f, eps, 0.0f)) - sdf_solid(p - vec3(0.0f, eps, 0.0f))) / (2.0f * eps)), ((sdf_solid(p + vec3(0.0f, 0.0f, eps)) - sdf_solid(p - vec3(0.0f, 0.0f, eps))) / (2.0f * eps))));\n}\n\nfloat sdf_scene(vec3 p) {\n    return _op_union(sdf_solid(p), sdf_ceiling_light(p));\n}\n\nvec3 grad_sdf_scene(vec3 p) {\n    float eps = 0.01f;\n    return normalize(vec3(((sdf_scene(p + vec3(eps, 0.0f, 0.0f)) - sdf_scene(p - vec3(eps, 0.0f, 0.0f))) / (2.0f * eps)), ((sdf_scene(p + vec3(0.0f, eps, 0.0f)) - sdf_scene(p - vec3(0.0f, eps, 0.0f))) / (2.0f * eps)), ((sdf_scene(p + vec3(0.0f, 0.0f, eps)) - sdf_scene(p - vec3(0.0f, 0.0f, eps))) / (2.0f * eps))));\n}\n\nMaterial scene_material(vec3 _t) {\n    float _x = _t.x;\n    float _y = _t.y;\n    float _z = _t.z;\n    return conditional_material((abs(sdf_ceiling_light(vec3(_x, _y, _z))) < abs(sdf_solid(vec3(_x, _y, _z)))), ceiling_light_material, conditional_material((abs(sdf_ground(vec3(_x, _y, _z))) < abs(sdf_subject(vec3(_x, _y, _z)))), ground_material, solid_material));\n}\n\nRay scene_camera_ray(vec2 _t) {\n    vec2 _v = _t;\n    return Ray((Camera(cameraPosition, cameraForward, cameraGlobalUp, resolution)).position, normalize(((normalize((Camera(cameraPosition, cameraForward, cameraGlobalUp, resolution)).forward) + ((camera_uv(Camera(cameraPosition, cameraForward, cameraGlobalUp, resolution), _v)).x * camera_right(Camera(cameraPosition, cameraForward, cameraGlobalUp, resolution)))) + ((camera_uv(Camera(cameraPosition, cameraForward, cameraGlobalUp, resolution), _v)).y * camera_up(Camera(cameraPosition, cameraForward, cameraGlobalUp, resolution))))));\n}\n\nHit scene_raytrace(Ray _t) {\n    vec3 _origin = _t.origin;\n    vec3 _dir = normalize(_t.dir);\n    float _travel = 0.0f;\n    for (int i = 0; i < default_raytrace_config.max_steps; i++) {\n        vec3 _p = _origin + _dir * _travel;\n        float _d = sdf_scene(_p);\n        if (_d < default_raytrace_config.hit_threshold) {\n            vec3 _n = grad_sdf_scene(_p);\n            return Hit(_p, _n, _travel, true);\n        }\n        _travel += _d;\n        if (_travel > default_raytrace_config.max_travel) {\n            break;\n        }\n    }\n    vec3 _miss = _origin + _dir * _travel;\n    return Hit(_miss, vec3(0.0f, 1.0f, 0.0f), _travel, false);\n}\n\nvec3 scene_material_color(Hit _t) {\n    Hit _hit = _t;\n    return material_color(scene_material((_hit).position));\n}\n\nvec3 scene_material_emission(Hit _t) {\n    Hit _hit = _t;\n    return material_emission(scene_material((_hit).position));\n}\n\nfloat scene_material_reflectiveness(Hit _t) {\n    Hit _hit = _t;\n    return material_reflectiveness(scene_material((_hit).position));\n}\n\nvec3 scene_raycolor(Ray _t) {\n    Ray _ray = Ray(_t.origin, normalize(_t.dir));\n    vec3 _radiance = vec3(0.0f);\n    vec3 _throughput = vec3(1.0f);\n    for (int bounce = 0; bounce < default_raycolor_config.max_bounces; bounce++) {\n        Hit _hit = scene_raytrace(_ray);\n        if (!_hit.hit) {\n            _radiance += _throughput * ambientColor;\n            break;\n        }\n        vec3 _surface_color = scene_material_color(_hit);\n        vec3 _surface_emission = scene_material_emission(_hit);\n        float _reflectiveness = clamp(scene_material_reflectiveness(_hit), 0.0f, 1.0f);\n        _radiance += _throughput * (_surface_emission + ((1.0f - _reflectiveness) * ambientColor * _surface_color));\n        _throughput *= _surface_color * _reflectiveness;\n        if (max(max(_throughput.r, _throughput.g), _throughput.b) < default_raycolor_config.throughput_threshold) {\n            break;\n        }\n        vec3 _dir = reflect(_ray.dir, _hit.normal);\n        _ray = Ray(_hit.position + _hit.normal * default_raycolor_config.ray_bias, _dir);\n    }\n    return _radiance;\n}\n\nvec4 scene_shade(vec2 _t) {\n    vec2 _v = _t;\n    return vec4(scene_raycolor(scene_camera_ray(_v)), 1.0f);\n}\n\nvoid main() {\n    outColor = scene_shade(gl_FragCoord.xy);\n}\n";

function setStatus(ok, message) {
    const status = document.getElementById("status") || document.createElement("pre");
    status.id = "status";
    status.textContent = `${ok ? "PASS" : "FAIL"} ${message}`;
    status.dataset.ok = ok ? "true" : "false";
    document.body.appendChild(status);
    if (!ok) {
        console.error(message);
    }
}

function compileShader(gl, type, source) {
    const shader = gl.createShader(type);
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
        const log = gl.getShaderInfoLog(shader) || "unknown shader compile error";
        gl.deleteShader(shader);
        throw new Error(log);
    }
    return shader;
}

function linkProgram(gl, vertexShader, fragmentShader) {
    const program = gl.createProgram();
    gl.attachShader(program, vertexShader);
    gl.attachShader(program, fragmentShader);
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
        const log = gl.getProgramInfoLog(program) || "unknown program link error";
        gl.deleteProgram(program);
        throw new Error(log);
    }
    return program;
}

function setUniforms(gl, program, width, height, time) {
    const setters = {
        cameraPosition: () => gl.uniform3f(location, 0.0, 0.0, -1.35),
        cameraForward: () => gl.uniform3f(location, 0.0, 0.0, 1.0),
        cameraGlobalUp: () => gl.uniform3f(location, 0.0, 1.0, 0.0),
        resolution: () => gl.uniform2f(location, width, height),
        ambientColor: () => gl.uniform3f(location, 0.035, 0.04, 0.055),
        time: () => gl.uniform1f(location, time),
        res: () => gl.uniform2f(location, width, height),
        scale: () => gl.uniform1f(location, 4.0),
    };

    for (const [name, setter] of Object.entries(setters)) {
        var location = gl.getUniformLocation(program, name);
        if (location !== null) {
            setter();
        }
    }
}

function installFallbackPositionBuffer(gl, program) {
    const location = gl.getAttribLocation(program, "a_position");
    if (location < 0) {
        return 3;
    }

    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(
        gl.ARRAY_BUFFER,
        new Float32Array([-1, -1, 3, -1, -1, 3]),
        gl.STATIC_DRAW,
    );
    gl.enableVertexAttribArray(location);
    gl.vertexAttribPointer(location, 2, gl.FLOAT, false, 0, 0);
    return 3;
}

function main() {
    const canvas = document.getElementById("canvas") || document.createElement("canvas");
    canvas.id = "canvas";
    document.body.style.margin = "0";
    document.body.style.background = "#0d0f14";
    canvas.style.display = "block";
    document.body.appendChild(canvas);
    const hasExplicitResolution =
        canvas.hasAttribute("width") || canvas.hasAttribute("height");
    if (!hasExplicitResolution) {
        canvas.style.width = "100vw";
        canvas.style.height = "100vh";
        const rect = canvas.getBoundingClientRect();
        const pixelRatio = Number(canvas.dataset.pixelRatio || window.devicePixelRatio || 1);
        canvas.width = Math.max(1, Math.floor(rect.width * pixelRatio));
        canvas.height = Math.max(1, Math.floor(rect.height * pixelRatio));
    }

    const gl = canvas.getContext("webgl2");
    if (!gl) {
        setStatus(false, "WebGL2 not supported");
        return;
    }

    try {
        const vertexShader = compileShader(gl, gl.VERTEX_SHADER, vertexShaderSource);
        const fragmentShader = compileShader(gl, gl.FRAGMENT_SHADER, fragmentShaderSource);
        const program = linkProgram(gl, vertexShader, fragmentShader);

        gl.useProgram(program);
        const width = gl.drawingBufferWidth;
        const height = gl.drawingBufferHeight;
        const count = installFallbackPositionBuffer(gl, program);

        gl.viewport(0, 0, width, height);
        const pixel = new Uint8Array(4);
        const start = performance.now();
        let sampled = false;
        function draw(now) {
            setUniforms(gl, program, width, height, (now - start) * 0.001);
            gl.clearColor(0, 0, 0, 1);
            gl.clear(gl.COLOR_BUFFER_BIT);
            gl.drawArrays(gl.TRIANGLES, 0, count);
            if (!sampled) {
                sampled = true;
                gl.readPixels(
                    Math.floor(width / 2),
                    Math.floor(height / 2),
                    1,
                    1,
                    gl.RGBA,
                    gl.UNSIGNED_BYTE,
                    pixel,
                );
                const visible = pixel[0] !== 0 || pixel[1] !== 0 || pixel[2] !== 0;
                setStatus(visible, `pixel=${Array.from(pixel).join(",")}`);
            }
            requestAnimationFrame(draw);
        }
        draw(performance.now());
    } catch (error) {
        setStatus(false, error.message);
    }
}

window.addEventListener("load", main);
