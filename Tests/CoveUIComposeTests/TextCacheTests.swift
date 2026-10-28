import Testing
@testable import CoveUICompose
@testable import CoveUIText

@Suite("Bounded text caches")
struct TextCacheTests {
    @Test func evictsLeastRecentlyReadEntry() {
        let cache = TextCache<String, Int>(limits: .init(maximumEntries: 2, maximumBytes: 10))
        func read(_ key: String, _ value: Int) -> Int {
            cache.value(for: key, cost: { _ in 4 }, build: { value })
        }
        #expect(read("a", 1) == 1)
        #expect(read("b", 2) == 2)
        #expect(read("a", 9) == 1)
        #expect(read("c", 3) == 3)
        #expect(read("a", 9) == 1)
        #expect(read("b", 9) == 9)
        #expect(cache.snapshot.entries == 2)
        #expect(cache.snapshot.estimatedBytes == 8)
    }

    @Test func oversizedResultsAndDisabledCachesAreNotRetained() {
        for limits in [TextCacheLimits(maximumEntries: 0), .init(maximumBytes: 3)] {
            let cache = TextCache<Int, Int>(limits: limits)
            #expect(cache.value(for: 1, cost: { _ in 4 }, build: { 42 }) == 42)
            #expect(cache.snapshot.entries == 0)
        }
    }

    @Test func trimDoesNotRetainAnOlderBuild() {
        let cache = TextCache<Int, Int>(limits: .init())
        let value = cache.value(for: 1, cost: { _ in 4 }) {
            cache.clear()
            return 42
        }
        #expect(value == 42)
        #expect(cache.snapshot.entries == 0)
    }

    @Test func dynamicTextAndWidthChangesRemainBounded() { GlobalTestLock.locked {
        let environment = TestTextEnvironmentFactory.make()
        for index in 0..<2_000 {
            _ = environment.cachedLayout(text: "Traffic \(index)", maxWidth: Float(100 + index % 400))
        }
        #expect(environment.shapingCacheSnapshot.entries <= 512)
        #expect(environment.layoutCacheSnapshot.entries <= 512)
        #expect(environment.layoutCacheSnapshot.estimatedBytes <= 8 * 1_024 * 1_024)
        environment.trimTextCaches()
        #expect(environment.shapingCacheSnapshot.entries == 0)
        #expect(environment.layoutCacheSnapshot.estimatedBytes == 0)
        #expect(!environment.cachedLayout(text: "Restored").lines.isEmpty)
    } }
}
