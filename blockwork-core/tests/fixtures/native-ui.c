#include <gtk/gtk.h>
static GtkWidget *status;
static void pressed(GtkButton *button, gpointer data) { gtk_label_set_text(GTK_LABEL(status), "pressed"); }
static void named(GtkWidget *widget, const char *name) { atk_object_set_name(gtk_widget_get_accessible(widget), name); }
int main(int argc, char **argv) {
    gtk_init(&argc, &argv);
    g_set_application_name("Blockwork native UI fixture");
    GtkWidget *window=gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(window), "Blockwork native UI fixture");
    gtk_window_set_default_size(GTK_WINDOW(window), 480, 420);
    GtkWidget *box=gtk_box_new(GTK_ORIENTATION_VERTICAL, 12);
    gtk_container_add(GTK_CONTAINER(window), box);
    GtkWidget *button=gtk_button_new_with_label("Activate"); named(button,"Activate");
    status=gtk_label_new("ready"); named(status,"Status");
    g_signal_connect(button,"clicked",G_CALLBACK(pressed),NULL);
    gtk_box_pack_start(GTK_BOX(box),button,FALSE,FALSE,0);
    gtk_box_pack_start(GTK_BOX(box),status,FALSE,FALSE,0);
    GtkWidget *entry=gtk_entry_new(); named(entry,"Input"); gtk_entry_set_text(GTK_ENTRY(entry),"initial");
    gtk_box_pack_start(GTK_BOX(box),entry,FALSE,FALSE,0);
    GtkWidget *slider=gtk_scale_new_with_range(GTK_ORIENTATION_HORIZONTAL,0,10,0.1); named(slider,"Amount");
    gtk_range_set_value(GTK_RANGE(slider),2.0); gtk_box_pack_start(GTK_BOX(box),slider,FALSE,FALSE,0);
    GtkWidget *password=gtk_entry_new(); gtk_entry_set_visibility(GTK_ENTRY(password),FALSE); named(password,"Secret"); gtk_entry_set_text(GTK_ENTRY(password),"protected-fixture-value");
    gtk_box_pack_start(GTK_BOX(box),password,FALSE,FALSE,0);
    for(int i=0;i<2;i++) { GtkWidget *duplicate=gtk_button_new_with_label("Duplicate"); named(duplicate,"Duplicate"); gtk_box_pack_start(GTK_BOX(box),duplicate,FALSE,FALSE,0); }
    GtkWidget *disabled=gtk_button_new_with_label("Disabled"); named(disabled,"Disabled"); gtk_widget_set_sensitive(disabled,FALSE); gtk_box_pack_start(GTK_BOX(box),disabled,FALSE,FALSE,0);
    g_signal_connect(window,"destroy",G_CALLBACK(gtk_main_quit),NULL);
    gtk_widget_show_all(window); gtk_main(); return 0;
}
