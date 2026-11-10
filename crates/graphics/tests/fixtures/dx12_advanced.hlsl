RWByteAddressBuffer result : register(u0);
RaytracingAccelerationStructure scene : register(t1);

[numthreads(1, 1, 1)]
void computeMain(uint3 thread : SV_DispatchThreadID) {
    result.Store(thread.x * 4, thread.x + 7);
}

[numthreads(1, 1, 1)]
void rayMain(uint3 thread : SV_DispatchThreadID) {
    RayDesc ray;
    ray.Origin = float3(thread.x * 3, 0, -1);
    ray.Direction = float3(0, 0, 1);
    ray.TMin = 0;
    ray.TMax = 10;
    RayQuery<RAY_FLAG_FORCE_OPAQUE> query;
    query.TraceRayInline(scene, RAY_FLAG_NONE, 0xff, ray);
    while (query.Proceed()) {}
    result.Store(thread.x * 4, query.CommittedStatus() == COMMITTED_TRIANGLE_HIT);
}

struct Vertex {
    float4 position : SV_Position;
};

Vertex vertexMain(uint id : SV_VertexID) {
    float2 p[3] = {float2(-1, -1), float2(1, -1), float2(0, 1)};
    Vertex output;
    output.position = float4(p[id], 0, 1);
    return output;
}

float4 fragmentMain() : SV_Target {
    return float4(0, 1, 0, 1);
}

[numthreads(3, 1, 1)]
[outputtopology("triangle")]
void meshMain(uint tid : SV_GroupIndex, out vertices Vertex verts[3], out indices uint3 tris[1]) {
    SetMeshOutputCounts(3, 1);
    verts[tid] = vertexMain(tid);
    if (tid == 0) tris[0] = uint3(0, 1, 2);
}

struct MeshPayload {
    float offset;
};
groupshared MeshPayload taskPayload;

[numthreads(1, 1, 1)]
void taskMain() {
    taskPayload.offset = 0.1;
    DispatchMesh(1, 1, 1, taskPayload);
}

[numthreads(3, 1, 1)]
[outputtopology("triangle")]
void taskMeshMain(in payload MeshPayload input, uint tid : SV_GroupIndex,
                  out vertices Vertex verts[3], out indices uint3 tris[1]) {
    SetMeshOutputCounts(3, 1);
    Vertex vertex = vertexMain(tid);
    vertex.position.x += input.offset;
    verts[tid] = vertex;
    if (tid == 0) tris[0] = uint3(0, 1, 2);
}
