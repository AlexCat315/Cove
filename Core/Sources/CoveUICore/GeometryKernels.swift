private import CRenderKernels
#if canImport(Dispatch) && !os(WASI)
import Dispatch
#endif
import Foundation

/// Borrows are joined before returning to Array. Workers only touch disjoint
/// ranges; no scene objects, painters or Array mutations cross threads.
private struct GeometryBuffer: @unchecked Sendable {
    let destination: UnsafeMutableRawPointer
}

enum GeometryKernels {
    #if canImport(Dispatch) && !os(WASI)
    private static let workers = min(8, ProcessInfo.processInfo.activeProcessorCount)
    private static let minimumChunk = 16_384
    #endif

    private static func chunks(count: Int, _ body: @Sendable (Int, Int) -> Void) {
        #if canImport(Dispatch) && !os(WASI)
        let jobs = min(workers, count / minimumChunk)
        guard jobs > 1 else { body(0, count); return }
        DispatchQueue.concurrentPerform(iterations: jobs) { job in
            let start = count * job / jobs
            body(start, count * (job + 1) / jobs - start)
        }
        #else
        // The browser prototype's WASI runtime has no worker threads.
        body(0, count)
        #endif
    }

    static func translate(_ destination: UnsafeMutablePointer<UIVertex>, count: Int, x: Float, y: Float) {
        precondition(MemoryLayout<UIVertex>.stride == MemoryLayout<CoveRenderVertex>.stride)
        let buffers = GeometryBuffer(destination: UnsafeMutableRawPointer(destination))
        chunks(count: count) { start, count in
            cove_render_translate(buffers.destination.advanced(by: start * UIVertex.stride), count, x, y)
        }
    }

    static func indices(_ destination: UnsafeMutablePointer<UInt32>, count: Int, offset: UInt32) {
        let buffers = GeometryBuffer(destination: UnsafeMutableRawPointer(destination))
        chunks(count: count) { start, count in
            cove_render_indices(buffers.destination.assumingMemoryBound(to: UInt32.self).advanced(by: start), count, offset)
        }
    }

    static func opacity(_ vertices: UnsafeMutablePointer<UIVertex>, count: Int, alpha: Float) {
        let buffer = GeometryBuffer(destination: UnsafeMutableRawPointer(vertices))
        chunks(count: count) { start, count in
            cove_render_opacity(buffer.destination.advanced(by: start * UIVertex.stride), count, alpha)
        }
    }

    static func bounds(_ vertices: [UIVertex]) -> UIRect? {
        guard !vertices.isEmpty else { return nil }
        return withUnsafeTemporaryAllocation(of: Float.self, capacity: 4) { bounds in
            vertices.withUnsafeBufferPointer { cove_render_bounds($0.baseAddress, $0.count, bounds.baseAddress) }
            return UIRect(x: bounds[0], y: bounds[1], width: bounds[2], height: bounds[3])
        }
    }
}
