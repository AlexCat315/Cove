// Keep the slots, sample count and dispatch width in sync with ray_query.rs.
RWByteAddressBuffer shadowMask : register(u0);
RaytracingAccelerationStructure occluders : register(t1);
cbuffer Scene : register(b2) {
    float4 lights[8];
    float4 floorGrid; // minimum x/z, grid spacing, grid width
};

[numthreads(8, 8, 1)]
void shadowMain(uint3 thread : SV_DispatchThreadID) {
    float2 floorPosition = floorGrid.x + (float2(thread.xy) + 0.5) * floorGrid.y;
    RayDesc ray;
    ray.Origin = float3(floorPosition.x, 0.001, floorPosition.y);
    ray.TMin = 0.001;
    uint blocked = 0;
    for (uint i = 0; i < 8; ++i) {
        float3 toLight = lights[i].xyz - ray.Origin;
        ray.TMax = length(toLight);
        ray.Direction = toLight / ray.TMax;
        RayQuery<RAY_FLAG_FORCE_OPAQUE | RAY_FLAG_ACCEPT_FIRST_HIT_AND_END_SEARCH> query;
        query.TraceRayInline(occluders, RAY_FLAG_NONE, 0xff, ray);
        while (query.Proceed()) {}
        blocked += query.CommittedStatus() == COMMITTED_TRIANGLE_HIT ? 1 : 0;
    }
    shadowMask.Store(4 * (thread.y * uint(floorGrid.z) + thread.x), blocked);
}
