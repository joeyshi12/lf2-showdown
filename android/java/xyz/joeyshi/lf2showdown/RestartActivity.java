package xyz.joeyshi.lf2showdown;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.os.Process;

/**
 * Started by MainActivity as it ends its process for a recreation; runs in a process
 * of its own (see AndroidManifest.xml), starts MainActivity in a new game process,
 * then ends.
 */
public class RestartActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        startActivity(new Intent(this, MainActivity.class)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
        finish();
        Process.killProcess(Process.myPid());
    }
}
