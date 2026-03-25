
// a + bi + cj + dk
vec4 multH(vec4 p, vec4 q) {
    return vec4(
        p.x * q.x - p.y * q.y - p.z * q.z - p.w * q.w,
        p.x * q.y + p.y * q.x + p.z * q.w - p.w * q.z,
        p.x * q.z - p.y * q.w + p.z * q.x + p.w * q.y,
        p.x * q.w + p.y * q.z - p.z * q.y + p.w * q.x
    );
}
// a + bi + cj + dk

vec4 conjH(vec4 q) {
    return vec4(q.x, -q.y, -q.z, -q.w);
}

vec4 inverseH(vec4 q) {
    float normSq = dot(q, q);
    return conjH(q) / normSq;
}

float realH(vec4 q) {
    return q.x;
}

vec3 imagH(vec4 q) {
    return q.yzw;
}

vec4 one_H = vec4(1.0, 0.0, 0.0, 0.0);
vec4 i_H = vec4(0.0, 1.0, 0.0, 0.0);
vec4 j_H = vec4(0.0, 0.0, 1.0, 0.0);
vec4 k_H = vec4(0.0, 0.0, 0.0, 1.0);

vec3 rotateVectorByQuaternion(vec3 v, vec4 q) {
    vec4 qv = vec4(0.0, v);
    vec4 qConj = conjH(q);
    vec4 rotatedQ = multH(multH(q, qv), qConj);
    return rotatedQ.yzw;
}

mat3 doubleCoverSO3(vec4 q) {
    float x0 = q.x;
    float x1 = -q.y;
    float x2 = -q.z;
    float x3 = -q.w;

    return mat3(
       x0*x0 + x1*x1 - x2*x2 - x3*x3, 2.0*(x1*x2 - x0*x3), 2.0*(x1*x3 + x0*x2),
         2.0*(x1*x2 + x0*x3), x0*x0 - x1*x1 + x2*x2 - x3*x3, 2.0*(x2*x3 - x0*x1),
         2.0*(x1*x3 - x0*x2), 2.0*(x2*x3 + x0*x1), x0*x0 - x1*x1 - x2*x2 + x3*x3
    );
}


struct SE3 {
    vec4 q;
    vec3 t;
};

SE3 multSE3(SE3 a, SE3 b) {
    return SE3(
        multH(a.q, b.q),
        a.t + rotateVectorByQuaternion(b.t, a.q)
    );
}

SE3 inv_SE3(SE3 a) {
    vec4 qInv = conjH(a.q);
    return SE3(
        conjH(a.q),
        rotateVectorByQuaternion(-a.t, conjH(a.q))
    );
}

SE3 id_SE3() {
    return SE3(one_H, vec3(0.0));
}

vec3 act_SE3(SE3 tf, vec3 p) {
    return rotateVectorByQuaternion(p, tf.q) + tf.t;
}
