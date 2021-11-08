#include "shader/common.inc"

Texture2D<float4> gScrgbTexture: register(t0);
RWTexture2D<float4> gHdr10Texture: register(u0);

[numthreads(8,8,1)]
void convert(uint3 id: SV_DispatchThreadID) {
    const float3x3 srgbToRec2020 = {
        0.6274, 0.3293, 0.0433,
        0.0691, 0.9195, 0.0114,
        0.0164, 0.0880, 0.8956,
    };
    uint2 dims;

    gHdr10Texture.GetDimensions(dims.x, dims.y);

    if (id.x >= dims.x || id.y >= dims.y) {
        return;
    }
    float4 scrgbColor = gScrgbTexture[id.xy];
    float3 rec2020LinearColor = (80.0 / 10000.0) * mul(srgbToRec2020, scrgbColor.rgb);

    gHdr10Texture[id.xy] = float4(
        inversePq(rec2020LinearColor.r),
        inversePq(rec2020LinearColor.g),
        inversePq(rec2020LinearColor.b),
        1.0
    );
}