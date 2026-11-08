package com.cove.probe;

import android.app.Activity;
import android.content.Context;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.content.res.Configuration;
import android.graphics.Rect;
import android.os.Build;
import android.view.Surface;
import androidx.core.util.Consumer;
import androidx.window.java.layout.WindowInfoTrackerCallbackAdapter;
import androidx.window.layout.DisplayFeature;
import androidx.window.layout.FoldingFeature;
import androidx.window.layout.WindowInfoTracker;
import androidx.window.layout.WindowLayoutInfo;
import java.util.ArrayList;
import java.util.List;

/**
 * Folds and hinges from Jetpack WindowManager. A foldable keeps its class
 * while folded: the cover display reports no feature, so the kind seen last is
 * remembered across launches.
 */
final class FoldingState {
    static final int PHONE = 0, TABLET = 1, BOOK = 2, FLIP = 3, DUAL_SCREEN = 4;
    static final int FLAT = 0, TABLETOP = 1, BOOK_POSTURE = 2;
    /** Matches `FEATURE_STRIDE` in the Rust decoder. */
    static final int STRIDE = 8;

    private static final String PREFERENCES = "com.cove.device";
    private static final String FOLDABLE_KIND = "foldableKind";

    private final Activity activity;
    private final WindowInfoTrackerCallbackAdapter tracker;
    private final Consumer<WindowLayoutInfo> listener;
    private final Runnable changed;
    private List<FoldingFeature> features = new ArrayList<>();
    private boolean listening;

    FoldingState(Activity activity, Runnable changed) {
        this.activity = activity;
        this.changed = changed;
        this.tracker = new WindowInfoTrackerCallbackAdapter(WindowInfoTracker.getOrCreate(activity));
        this.listener = info -> {
            List<FoldingFeature> next = new ArrayList<>();
            for (DisplayFeature feature : info.getDisplayFeatures()) {
                if (feature instanceof FoldingFeature) next.add((FoldingFeature) feature);
            }
            features = next;
            remember();
            changed.run();
        };
    }

    void start() {
        if (listening) return;
        tracker.addWindowLayoutInfoListener(activity, activity.getMainExecutor(), listener);
        listening = true;
    }

    void stop() {
        if (!listening) return;
        tracker.removeWindowLayoutInfoListener(listener);
        listening = false;
    }

    int deviceClass(Configuration configuration) {
        int kind = currentKind();
        if (kind >= 0) return kind;
        int remembered = preferences().getInt(FOLDABLE_KIND, -1);
        if (remembered >= 0) return remembered;
        if (hasHingeSensor()) return BOOK;
        return configuration.smallestScreenWidthDp >= 600 ? TABLET : PHONE;
    }

    int posture() {
        for (FoldingFeature feature : features) {
            if (feature.getState() != FoldingFeature.State.HALF_OPENED) continue;
            return feature.getOrientation() == FoldingFeature.Orientation.HORIZONTAL ? TABLETOP : BOOK_POSTURE;
        }
        return FLAT;
    }

    /** `kind, x, y, width, height, orientation, occludes, separates` in logical points. */
    float[] features(float density) {
        float[] values = new float[features.size() * STRIDE];
        int index = 0;
        for (FoldingFeature feature : features) {
            Rect bounds = feature.getBounds();
            boolean occludes = feature.getOcclusionType() == FoldingFeature.OcclusionType.FULL;
            values[index++] = occludes ? 1 : 0;
            values[index++] = bounds.left / density;
            values[index++] = bounds.top / density;
            values[index++] = bounds.width() / density;
            values[index++] = bounds.height() / density;
            values[index++] = feature.getOrientation() == FoldingFeature.Orientation.HORIZONTAL ? 0 : 1;
            values[index++] = occludes ? 1 : 0;
            values[index++] = feature.isSeparating() ? 1 : 0;
        }
        return values;
    }

    private int currentKind() {
        for (FoldingFeature feature : features) {
            if (feature.getOcclusionType() == FoldingFeature.OcclusionType.FULL) return DUAL_SCREEN;
            boolean vertical = feature.getOrientation() == FoldingFeature.Orientation.VERTICAL;
            int rotation = rotation();
            boolean rotated = rotation == Surface.ROTATION_90 || rotation == Surface.ROTATION_270;
            // In the display's natural orientation a book folds vertically and
            // a flip horizontally.
            return vertical != rotated ? BOOK : FLIP;
        }
        return -1;
    }

    private void remember() {
        int kind = currentKind();
        if (kind < 0) return;
        SharedPreferences preferences = preferences();
        if (preferences.getInt(FOLDABLE_KIND, -1) != kind) preferences.edit().putInt(FOLDABLE_KIND, kind).apply();
    }

    private int rotation() {
        if (Build.VERSION.SDK_INT >= 30 && activity.getDisplay() != null) return activity.getDisplay().getRotation();
        @SuppressWarnings("deprecation")
        int legacy = activity.getWindowManager().getDefaultDisplay().getRotation();
        return legacy;
    }

    private boolean hasHingeSensor() {
        return Build.VERSION.SDK_INT >= 30
                && activity.getPackageManager().hasSystemFeature(PackageManager.FEATURE_SENSOR_HINGE_ANGLE);
    }

    private SharedPreferences preferences() {
        return activity.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE);
    }
}
