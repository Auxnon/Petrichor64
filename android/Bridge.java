// The little Java the Android build can't do without.
//
// The app is a NativeActivity with no classes of its own in the APK, and two
// things only reach a Java object: a drag from another app (a View's drag
// listener) and the answer from the system's file picker (onActivityResult).
// build.rs compiles this file to a dex and embeds it in the library;
// src/droid.rs loads it from memory at startup and registers `onFile`.
//
// Both hand the engine a path, and what Android gives is a content URI, so
// the document is copied into the app's cache first.
package com.makeavoy.petrichor64;

import android.app.Activity;
import android.app.Fragment;
import android.content.ClipData;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.util.Log;
import android.view.DragEvent;
import android.view.View;
import android.view.ViewGroup;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;

public final class Bridge {
    static final String TAG = "petrichor64";
    static final String PICKER = "petrichor64-picker";

    /** A file is ready at `path`. Implemented in src/droid.rs. */
    static native void onFile(String path);

    /** Take drags from other apps (split screen, a files app) as drops. */
    public static void install(final Activity activity) {
        activity.runOnUiThread(new Runnable() {
            public void run() {
                // NativeActivity's content view covers the window; it is the
                // content frame's only child.
                View target = activity.findViewById(android.R.id.content);
                if (target instanceof ViewGroup && ((ViewGroup) target).getChildCount() > 0) {
                    target = ((ViewGroup) target).getChildAt(0);
                }
                if (target == null) {
                    target = activity.getWindow().getDecorView();
                }
                target.setOnDragListener(new View.OnDragListener() {
                    public boolean onDrag(View v, DragEvent e) {
                        if (e.getAction() != DragEvent.ACTION_DROP) {
                            // Saying yes to the start is what makes this view
                            // a drop target at all.
                            return true;
                        }
                        // Without this the URIs can't be read: they belong
                        // to the app the drag came from.
                        activity.requestDragAndDropPermissions(e);
                        ClipData clip = e.getClipData();
                        if (clip == null) {
                            return false;
                        }
                        for (int i = 0; i < clip.getItemCount(); i++) {
                            Uri uri = clip.getItemAt(i).getUri();
                            if (uri != null) {
                                take(activity, uri);
                            }
                        }
                        return true;
                    }
                });
            }
        });
    }

    /** Show the system's document picker; what's chosen arrives at onFile. */
    public static void pick(final Activity activity) {
        activity.runOnUiThread(new Runnable() {
            public void run() {
                android.app.FragmentManager fm = activity.getFragmentManager();
                Fragment old = fm.findFragmentByTag(PICKER);
                android.app.FragmentTransaction t = fm.beginTransaction();
                if (old != null) {
                    t.remove(old);
                }
                t.add(new Picker(), PICKER);
                t.commitAllowingStateLoss();
            }
        });
    }

    /**
     * Only an Activity or a Fragment is told a picker's result, and the
     * activity is the framework's own, so a fragment with no view asks.
     *
     * If Android kills the app while the picker is up it will try to bring
     * this fragment back before the engine has loaded this class, and fail:
     * the app then starts again from scratch, which is what it would do
     * anyway.
     */
    public static final class Picker extends Fragment {
        @Override
        public void onCreate(Bundle saved) {
            super.onCreate(saved);
            if (saved != null) {
                return;
            }
            Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            intent.addCategory(Intent.CATEGORY_OPENABLE);
            // A `.game.png` is an image and a `.bin` has no type worth
            // naming, so nothing narrower than everything would do.
            intent.setType("*/*");
            try {
                startActivityForResult(intent, 1);
            } catch (Exception e) {
                Log.e(TAG, "no document picker: " + e);
            }
        }

        @Override
        public void onActivityResult(int request, int result, Intent data) {
            Activity activity = getActivity();
            if (activity == null) {
                return;
            }
            if (result == Activity.RESULT_OK && data != null && data.getData() != null) {
                take(activity, data.getData());
            }
            activity.getFragmentManager().beginTransaction().remove(this).commitAllowingStateLoss();
        }
    }

    /** Copy the document into the cache, off the UI thread, and report it. */
    static void take(final Activity activity, final Uri uri) {
        new Thread(new Runnable() {
            public void run() {
                try {
                    File dir = new File(activity.getCacheDir(), "dropped");
                    dir.mkdirs();
                    File out = new File(dir, name(activity, uri));
                    InputStream in = activity.getContentResolver().openInputStream(uri);
                    if (in == null) {
                        Log.e(TAG, "can't read " + uri);
                        return;
                    }
                    try {
                        OutputStream os = new FileOutputStream(out);
                        try {
                            byte[] buf = new byte[65536];
                            int n;
                            while ((n = in.read(buf)) > 0) {
                                os.write(buf, 0, n);
                            }
                        } finally {
                            os.close();
                        }
                    } finally {
                        in.close();
                    }
                    onFile(out.getAbsolutePath());
                } catch (Exception e) {
                    Log.e(TAG, "couldn't take " + uri + ": " + e);
                }
            }
        }).start();
    }

    /** The document's own file name: the engine reads the kind from it. */
    static String name(Activity activity, Uri uri) {
        String name = null;
        try {
            Cursor c = activity.getContentResolver().query(
                uri, new String[] { OpenableColumns.DISPLAY_NAME }, null, null, null);
            if (c != null) {
                try {
                    if (c.moveToFirst()) {
                        name = c.getString(0);
                    }
                } finally {
                    c.close();
                }
            }
        } catch (Exception e) {
            // Not every provider answers; the URI's tail will do.
        }
        if (name == null || name.isEmpty()) {
            name = uri.getLastPathSegment();
        }
        if (name == null || name.isEmpty()) {
            name = "dropped";
        }
        return name.replace('/', '_').replace('\\', '_');
    }
}
