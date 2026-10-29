package __ruffle__ {
    // AIR filesystem bridge (`flash.filesystem.File` / `FileStream`).
    public native function fs_read(path:String):*;
    public native function fs_write(path:String, bytes:*):Boolean;
    public native function fs_exists(path:String):Boolean;
    public native function fs_is_directory(path:String):Boolean;
    public native function fs_size(path:String):Number;
    public native function fs_list_directory(path:String):Array;
    public native function fs_create_directory(path:String):Boolean;
    public native function fs_delete(path:String):Boolean;
    public native function fs_directory(which:String):String;

    // Decode a Brawlhalla `.swz` container into an Array of entry strings.
    public native function swz_decode(bytes:*, key:uint):Array;
}
