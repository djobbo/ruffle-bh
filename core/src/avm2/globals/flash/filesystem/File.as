package flash.filesystem {
    import __ruffle__.fs_create_directory;
    import __ruffle__.fs_delete;
    import __ruffle__.fs_directory;
    import __ruffle__.fs_exists;
    import __ruffle__.fs_is_directory;
    import __ruffle__.fs_list_directory;
    import __ruffle__.fs_size;
    import __ruffle__.stub_method;

    import flash.net.FileReference;
    import flash.events.FileListEvent;
    import flash.utils.getTimer;
    import flash.utils.setTimeout;

    /**
     * AIR `File`. Ruffle exposes a host-backed filesystem (the web player
     * serves files over HTTP; the desktop player uses the OS filesystem).
     */
    public class File extends FileReference {
        private static var _applicationDirectory:File = null;
        private static var _applicationStorageDirectory:File = null;
        private static var _documentsDirectory:File = null;
        private static var _userDirectory:File = null;
        private static var _desktopDirectory:File = null;

        private var _path:String = "";

        public function File(path:String = null) {
            if (path != null) {
                this._path = normalize(path);
            }
        }

        private static function normalize(path:String):String {
            if (path == null) {
                return "";
            }
            var slashed:String = path.replace(/\\/g, "/");
            var absolute:Boolean = slashed.charAt(0) == "/";
            var parts:Array = slashed.split("/");
            var stack:Array = [];
            for (var i:int = 0; i < parts.length; i++) {
                var part:String = parts[i];
                if (part == "" || part == ".") {
                    continue;
                }
                if (part == "..") {
                    if (stack.length > 0) {
                        stack.pop();
                    }
                    continue;
                }
                stack.push(part);
            }
            var normalized:String = stack.join("/");
            if (absolute) {
                return "/" + normalized;
            }
            return normalized;
        }

        private static function join(base:String, relative:String):String {
            if (relative == null || relative == "") {
                return base;
            }
            if (relative.charAt(0) == "/" || relative.indexOf(":") == 1) {
                return normalize(relative);
            }
            if (base == null || base == "") {
                return normalize(relative);
            }
            return normalize(base + "/" + relative);
        }

        public static function get applicationDirectory():File {
            if (_applicationDirectory === null) {
                _applicationDirectory = new File(fs_directory("application"));
            }
            return _applicationDirectory;
        }

        public static function get applicationStorageDirectory():File {
            if (_applicationStorageDirectory === null) {
                _applicationStorageDirectory = new File(fs_directory("storage"));
            }
            return _applicationStorageDirectory;
        }

        public static function get documentsDirectory():File {
            if (_documentsDirectory === null) {
                _documentsDirectory = new File(fs_directory("documents"));
            }
            return _documentsDirectory;
        }

        public static function get userDirectory():File {
            if (_userDirectory === null) {
                _userDirectory = new File(fs_directory("user"));
            }
            return _userDirectory;
        }

        public static function get desktopDirectory():File {
            if (_desktopDirectory === null) {
                _desktopDirectory = new File(fs_directory("desktop"));
            }
            return _desktopDirectory;
        }

        public static function get separator():String {
            return "/";
        }

        public function get nativePath():String {
            return this._path;
        }

        public function set nativePath(value:String):void {
            this._path = normalize(value);
        }

        public function get url():String {
            return "file://" + encodeURI(this._path);
        }

        override public function get name():String {
            var slash:int = this._path.lastIndexOf("/");
            return slash >= 0 ? this._path.substring(slash + 1) : this._path;
        }

        override public function get type():String {
            var name:String = this.name;
            var dot:int = name.lastIndexOf(".");
            return dot > 0 ? name.substring(dot) : null;
        }

        public function get parent():File {
            var slash:int = this._path.lastIndexOf("/");
            if (slash <= 0) {
                return null;
            }
            return new File(this._path.substring(0, slash));
        }

        override public function get size():Number {
            return fs_size(this._path);
        }

        public function get isDirectory():Boolean {
            return fs_is_directory(this._path);
        }

        public function get exists():Boolean {
            return fs_exists(this._path);
        }

        public function get isHidden():Boolean {
            return false;
        }

        public function get isSymbolicLink():Boolean {
            return false;
        }

        override public function get creationDate():Date {
            return new Date(0);
        }

        override public function get modificationDate():Date {
            return new Date(0);
        }

        public function get spaceAvailable():Number {
            return 0;
        }

        public function resolvePath(path:String):File {
            return new File(join(this._path, path));
        }

        public function getRelativePath(ref:File, useDotDot:Boolean = false):String {
            stub_method("flash.filesystem.File", "getRelativePath");
            return this._path;
        }

        public function clone():File {
            return new File(this._path);
        }

        public function canonicalize():void {
            this._path = normalize(this._path);
        }

        public function getDirectoryListing():Array {
            var names:Array = fs_list_directory(this._path);
            if (names == null) {
                return [];
            }
            var result:Array = [];
            for (var i:int = 0; i < names.length; i++) {
                result.push(this.resolvePath(String(names[i])));
            }
            return result;
        }

        public function getDirectoryListingAsync():void {
            var listing:Array = getDirectoryListing();
            setTimeout(function():void {
                dispatchEvent(new FileListEvent(FileListEvent.DIRECTORY_LISTING, false, false, listing));
            }, 1);
        }

        public function createDirectory():void {
            if (!fs_create_directory(this._path)) {
                throw new Error("Unable to create directory: " + this._path);
            }
        }

        public function deleteFile():void {
            if (!fs_delete(this._path)) {
                throw new Error("Unable to delete file: " + this._path);
            }
        }

        public function deleteDirectory(deleteDirectoryContents:Boolean = false):void {
            if (!fs_delete(this._path)) {
                throw new Error("Unable to delete directory: " + this._path);
            }
        }

        public function moveTo(newLocation:File, overwrite:Boolean = false):void {
            stub_method("flash.filesystem.File", "moveTo");
        }

        public function moveToAsync(newLocation:File, overwrite:Boolean = false):void {
            stub_method("flash.filesystem.File", "moveToAsync");
        }

        public function moveToTrash():void {
            this.deleteFile();
        }

        public function moveToTrashAsync():void {
            stub_method("flash.filesystem.File", "moveToTrashAsync");
        }

        public function copyTo(newLocation:File, overwrite:Boolean = false):void {
            stub_method("flash.filesystem.File", "copyTo");
        }

        public function copyToAsync(newLocation:File, overwrite:Boolean = false):void {
            stub_method("flash.filesystem.File", "copyToAsync");
        }

        public function preventBackup():void {
            // no-op: there is nothing to back up in the host filesystem
        }

        public function get icon():Object {
            return null;
        }
    }
}
