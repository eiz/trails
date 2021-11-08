float pq(float Ep) {
    const float m1 = 0.1593017578125;
    const float m2 = 78.84375;
    const float c1 = 0.8359375;
    const float c2 = 18.8515625;
    const float c3 = 18.6875;
    float logEp = pow(abs(Ep), 1.0/m2);

    return pow(abs(max(logEp - c1, 0) / (c2 - c3 * logEp)), 1.0/m1);
}

float inversePq(float Y) {
    const float m1 = 0.1593017578125;
    const float m2 = 78.84375;
    const float c1 = 0.8359375;
    const float c2 = 18.8515625;
    const float c3 = 18.6875;
    float Yp = pow(abs(Y), m1);

    return pow((c1 + c2 * Yp) / (1 + c3 * Yp), m2);
}