/* Private shared GSettings only. Never constructs the default/dconf backend. */
#define G_SETTINGS_ENABLE_BACKEND
#include <gio/gsettingsbackend.h>
#include <gio/gio.h>

int main(int argc, char **argv) {
  if (argc < 6 || (argc - 4) % 2) return 2;
  if (!g_path_is_absolute(argv[1])) return 2;
  GSettingsSchemaSource *source = g_settings_schema_source_get_default();
  GSettingsSchema *schema = g_settings_schema_source_lookup(source, argv[2], TRUE);
  if (!schema) return 3;
  GSettingsBackend *backend = g_keyfile_settings_backend_new(argv[1], "/", NULL);
  if (!backend) return 4;
  GSettings *settings = g_settings_new_full(schema, backend, g_str_equal(argv[3], "-") ? NULL : argv[3]);
  if (!settings) return 5;
  g_settings_delay(settings);
  for (int i=4; i<argc; i+=2) {
    if (!g_settings_schema_has_key(schema, argv[i])) return 6;
    if (g_str_equal(argv[i+1], "@reset")) {
      g_settings_reset(settings, argv[i]);
      continue;
    }
    GSettingsSchemaKey *key = g_settings_schema_get_key(schema, argv[i]);
    GError *error = NULL;
    GVariant *value = g_variant_parse(g_settings_schema_key_get_value_type(key), argv[i+1], NULL, NULL, &error);
    if (!value || !g_settings_schema_key_range_check(key, value) || !g_settings_set_value(settings, argv[i], value)) {
      g_printerr("Invalid private setting %s\n", argv[i]);
      return 7;
    }
    g_variant_unref(value);
    g_settings_schema_key_unref(key);
  }
  g_settings_apply(settings);
  g_settings_sync();
  g_object_unref(settings);
  g_object_unref(backend);
  g_settings_schema_unref(schema);
  return 0;
}
