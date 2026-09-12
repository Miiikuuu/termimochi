/* Minimal actual Casilda host. LGPL Casilda remains a separately linked library. */
#include <casilda.h>
#include <signal.h>
#include <unistd.h>
#include <sys/prctl.h>

static GtkApplication *app;
static GPid child;
static gchar **client_argv;
static void child_setup(gpointer unused) {
  (void)unused;
  setsid();
  prctl(PR_SET_PDEATHSIG, SIGTERM);
}
static void exited(GPid pid, gint status, gpointer data) {
  (void)data;
  g_print("CLIENT_EXIT pid=%d status=%d\n", pid, status);
  child = 0;
  g_spawn_close_pid(pid);
}
static gboolean close_host(GtkWindow *window, gpointer data) {
  (void)window; (void)data;
  if (child > 0) kill(-child, SIGTERM);
  return FALSE;
}
static void activate(GtkApplication *application, gpointer data) {
  (void)data;
  GtkWidget *window = gtk_application_window_new(application);
  gtk_window_set_title(GTK_WINDOW(window), "TermiMochi Native Host Probe");
  gtk_window_set_default_size(GTK_WINDOW(window), 1100, 760);
  GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 8);
  gtk_box_append(GTK_BOX(box), gtk_label_new("Actual native client below — commands execute; private configuration is NOT a filesystem sandbox"));
  GtkWidget *entry = gtk_entry_new();
  gtk_entry_set_placeholder_text(GTK_ENTRY(entry), "Host input / focus / clipboard check");
  gtk_box_append(GTK_BOX(box), entry);
  CasildaCompositor *compositor = casilda_compositor_new(NULL);
  gtk_widget_set_vexpand(GTK_WIDGET(compositor), TRUE);
  gtk_box_append(GTK_BOX(box), GTK_WIDGET(compositor));
  gtk_window_set_child(GTK_WINDOW(window), box);
  g_signal_connect(window, "close-request", G_CALLBACK(close_host), NULL);
  gtk_window_present(GTK_WINDOW(window));
  GError *error = NULL;
  gchar **env = g_get_environ();
  env = g_environ_setenv(env, "GTK_IM_MODULE", "wayland", TRUE);
  if (!casilda_compositor_spawn_async(compositor, NULL, client_argv, env,
      G_SPAWN_DO_NOT_REAP_CHILD, child_setup, NULL, &child, &error)) {
    g_printerr("SPAWN_FAILED %s\n", error->message);
    g_clear_error(&error);
    g_application_quit(G_APPLICATION(application));
    return;
  }
  g_strfreev(env);
  g_child_watch_add(child, exited, NULL);
  g_print("HOST_PID=%d CLIENT_PID=%d PRIVATE_WAYLAND_FD=1\n", getpid(), child);
}
int main(int argc, char **argv) {
  if (argc < 2) return 2;
  client_argv = &argv[1];
  app = gtk_application_new("io.github.miiikuuu.termimochi.NativeProbe", G_APPLICATION_NON_UNIQUE);
  g_signal_connect(app, "activate", G_CALLBACK(activate), NULL);
  int result = g_application_run(G_APPLICATION(app), 1, argv);
  if (child > 0) kill(-child, SIGTERM);
  g_object_unref(app);
  return result;
}
