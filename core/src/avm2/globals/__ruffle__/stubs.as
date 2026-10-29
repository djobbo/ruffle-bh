package __ruffle__ {
    // Mark the method where this function is invoked as stubbed.
    public native function stub_method(className:String, methodName:String, specifics:String = null):void;

    // Mark the getter where this function is invoked as stubbed.
    public native function stub_getter(className:String, propertyName:String):void;

    // Mark the setter where this function is invoked as stubbed.
    public native function stub_setter(className:String, propertyName:String):void;

    // Mark the constructor where this function is invoked as stubbed.
    public native function stub_constructor(className:String, specifics:String = null):void;

    // Note: the following function is not related to stubbing.

    // Produce a regular warning in Ruffle logs.
    public native function log_warn(...rest):void;

    // Headless replay exporter: append one hitbox/hurtbox rect to the dump file.
    public native function dump_box(
        x:Number,
        y:Number,
        w:Number,
        h:Number,
        worldX:Number,
        worldY:Number,
        powerFrame:uint,
        powerId:uint,
        kind:uint
    ):void;

    // Reset the native frame counter when a replay starts.
    public native function reset_dump_frame():void;

    // Headless replay exporter: dump one JSONL frame of player state.
    public native function dump_state(game:*, entities:*, time:uint):void;

    // One-shot: enter training after the game object is ready.
    public native function auto_start_training(game:*, time:uint):void;

    // One-shot: enable hitbox/hurtbox debug displays for a live match.
    public native function enable_debug(game:*, time:uint):void;

    // Command line exposed to the DesktopExtension ANE shim.
    public native function get_command_line():String;
}
