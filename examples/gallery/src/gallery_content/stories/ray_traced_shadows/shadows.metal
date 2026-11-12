#include <metal_stdlib>
#include <metal_raytracing>
using namespace metal;
using namespace raytracing;

struct Scene {
    float4 lights[8];
    float4 floorGrid;
};

kernel void shadowMain(device uint *shadowMask [[buffer(0)]],
                       instance_acceleration_structure occluders [[buffer(1)]],
                       constant Scene &scene [[buffer(2)]],
                       uint2 thread [[thread_position_in_grid]]) {
    float2 floorPosition = scene.floorGrid.x + (float2(thread) + 0.5f) * scene.floorGrid.y;
    ray shadowRay;
    shadowRay.origin = float3(floorPosition.x, 0.001f, floorPosition.y);
    shadowRay.min_distance = 0.001f;
    intersector<triangle_data, instancing> tracer;
    tracer.accept_any_intersection(true);
    uint blocked = 0;
    for (uint i = 0; i < 8; ++i) {
        float3 toLight = scene.lights[i].xyz - shadowRay.origin;
        shadowRay.max_distance = length(toLight);
        shadowRay.direction = toLight / shadowRay.max_distance;
        blocked += tracer.intersect(shadowRay, occluders).type != intersection_type::none ? 1 : 0;
    }
    shadowMask[thread.y * uint(scene.floorGrid.z) + thread.x] = blocked;
}
