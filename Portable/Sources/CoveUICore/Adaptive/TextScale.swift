extension UIEnvironment {
    /// A host's point-size mapping. Native accessibility ramps may scale small
    /// and large text differently; previews can also supply a linear factor.
    public struct TextScale: Sendable, Hashable {
        public struct Sample: Sendable, Hashable {
            public let size: Float
            public let scaledSize: Float
            public init(size: Float, scaledSize: Float) {
                self.size = size; self.scaledSize = scaledSize
            }
        }

        private let samples: [Sample]

        public init(_ factor: Float = 1) {
            samples = [Sample(size: 1, scaledSize: factor.isFinite && factor > 0 ? factor : 1)]
        }

        public init(samples: [Sample]) {
            let ordered = samples.filter { $0.size.isFinite && $0.size > 0 && $0.scaledSize.isFinite && $0.scaledSize > 0 }
                .sorted { $0.size < $1.size }
            let valid = !ordered.isEmpty && zip(ordered, ordered.dropFirst()).allSatisfy {
                $0.size < $1.size && $0.scaledSize <= $1.scaledSize
            }
            self.samples = valid ? ordered : [Sample(size: 1, scaledSize: 1)]
        }

        /// Representative body-text ratio, for icons and layout density.
        public var factor: Float { scale(16) / 16 }

        public func scale(_ size: Float) -> Float {
            guard size.isFinite else { return size }
            if size < 0 { return -scale(-size) }
            let first = samples[0]
            if size <= first.size { return size * (first.scaledSize / first.size) }
            for (lower, upper) in zip(samples, samples.dropFirst()) where size <= upper.size {
                let fraction = (size - lower.size) / (upper.size - lower.size)
                return lower.scaledSize + (upper.scaledSize - lower.scaledSize) * fraction
            }
            let last = samples[samples.count - 1]
            return size * (last.scaledSize / last.size)
        }
    }
}
