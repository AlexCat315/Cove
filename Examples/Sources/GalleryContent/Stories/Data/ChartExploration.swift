import Foundation
import CoveUI

struct ChartExplorationStory: View {
    @State private var range: ClosedRange<Float>?
    @State private var brushed: ClosedRange<Float>?
    @State private var revision = 0

    private var samples: [ChartDatum] {
        (0..<60).map { i in
            let x = Float(i)
            return ChartDatum(x: x, y: 25 + sinf(x * 0.2 + Float(revision)) * 14 + cosf(x * 0.7) * 3)
        }
    }

    var body: some View {
        StorySection("Explore", "Scroll to zoom, drag to brush, Alt-drag to pan.") {
            Box(direction: .row, spacing: 8) {
                Button("Focus") { range = 20...40 }.debugName("chart-zoom-focus")
                Button("Reset") { range = nil; brushed = nil }.debugName("chart-zoom-reset")
                Button("Update data") { revision += 1 }.debugName("chart-animate-data")
            }
            Chart([ChartSeries(points: samples, mark: .area, title: "Signal")], x: .numeric,
                  style: ChartStyle(minValue: 0, maxValue: 45)) {
                $0.label = "Explore signal"
                $0.viewport.range = $range
                $0.selection.range = $brushed
                $0.animation = .easeInOut(duration: 0.45)
            }
            .frame(height: 200).debugName("chart-exploration")
            if let brushed {
                Text(String(format: "Selected %.1f–%.1f", brushed.lowerBound, brushed.upperBound))
                    .font(.caption).debugName("chart-brushed-range")
            }
        }
    }
}
