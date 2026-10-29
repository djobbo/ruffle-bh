package flash.events {
    import flash.events.Event;

    [API("662")]
    public class FileListEvent extends Event {
        public static const DIRECTORY_LISTING:String = "directoryListing";
        public static const SELECT_MULTIPLE:String = "selectMultiple";

        /** Most recently constructed event, for 0-arg listener compatibility. */
        public static var lastEvent:FileListEvent;

        public var files:Array;

        public function FileListEvent(
            type:String,
            bubbles:Boolean = false,
            cancelable:Boolean = false,
            files:Array = null
        ) {
            super(type, bubbles, cancelable);
            this.files = files;
            lastEvent = this;
        }

        override public function clone():Event {
            return new FileListEvent(type, bubbles, cancelable, files);
        }
    }
}
