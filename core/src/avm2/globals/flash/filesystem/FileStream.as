package flash.filesystem {
    import __ruffle__.fs_read;
    import __ruffle__.fs_write;
    import __ruffle__.stub_method;

    import flash.errors.IOError;
    import flash.events.Event;
    import flash.events.EventDispatcher;
    import flash.net.ObjectEncoding;
    import flash.utils.ByteArray;
    import flash.utils.Endian;
    import flash.utils.IDataInput;
    import flash.utils.IDataOutput;

    /**
     * AIR `FileStream`, backed by the host filesystem.
     *
     * The whole file is read into memory on `open()` and written back on
     * `close()`; Brawlhalla only uses synchronous reads of read-only assets,
     * which makes this a faithful (and cheap) implementation.
     */
    public class FileStream extends EventDispatcher implements IDataInput, IDataOutput {
        private var _bytes:ByteArray = null;
        private var _path:String = null;
        private var _mode:String = null;
        private var _dirty:Boolean = false;
        private var _readAhead:Number = 0;
        private var _endian:String = Endian.BIG_ENDIAN;
        private var _objectEncoding:uint = ObjectEncoding.AMF3;

        public function FileStream() {
            super();
        }

        public function open(file:File, fileMode:String):void {
            if (this._bytes != null) {
                this.close();
            }

            var isWriteOnly:Boolean = fileMode == FileMode.WRITE;
            var data:ByteArray = null;
            if (!isWriteOnly) {
                data = fs_read(file.nativePath);
            }

            if (data == null) {
                if (fileMode == FileMode.READ) {
                    throw new IOError("File not found: " + file.nativePath);
                }
                data = new ByteArray();
            }

            this._bytes = data;
            this._path = file.nativePath;
            this._mode = fileMode;
            this._dirty = false;
            this._bytes.endian = this._endian;
            this._bytes.objectEncoding = this._objectEncoding;
            if (fileMode == FileMode.APPEND) {
                this._bytes.position = this._bytes.length;
            } else {
                this._bytes.position = 0;
            }
        }

        public function openAsync(file:File, fileMode:String):void {
            // Synchronous under the hood; still notify listeners like AIR would.
            this.open(file, fileMode);
            dispatchEvent(new Event(Event.OPEN));
            dispatchEvent(new Event(Event.COMPLETE));
        }

        public function close():void {
            if (this._bytes == null) {
                return;
            }
            if (this._dirty && this._mode != FileMode.READ && this._path != null) {
                fs_write(this._path, this._bytes);
            }
            this._bytes = null;
            this._path = null;
            this._mode = null;
            this._dirty = false;
            dispatchEvent(new Event(Event.CLOSE));
        }

        public function get bytesAvailable():uint {
            return this.requireBytes().bytesAvailable;
        }

        public function get position():Number {
            return this.requireBytes().position;
        }

        public function set position(value:Number):void {
            this.requireBytes().position = value;
        }

        public function get endian():String {
            return this._endian;
        }

        public function set endian(value:String):void {
            this._endian = value;
            if (this._bytes != null) {
                this._bytes.endian = value;
            }
        }

        public function get objectEncoding():uint {
            return this._objectEncoding;
        }

        public function set objectEncoding(value:uint):void {
            this._objectEncoding = value;
            if (this._bytes != null) {
                this._bytes.objectEncoding = value;
            }
        }

        public function get readAhead():Number {
            return this._readAhead;
        }

        public function set readAhead(value:Number):void {
            this._readAhead = value;
        }

        public function readAheadAsync(bytes:int = 8192):void {
            stub_method("flash.filesystem.FileStream", "readAheadAsync");
        }

        public function readAheadStop():void {
            // no-op: everything is already in memory
        }

        private function requireBytes():ByteArray {
            if (this._bytes == null) {
                throw new IOError("FileStream is not open");
            }
            return this._bytes;
        }

        private function afterWrite():void {
            this._dirty = true;
            // WRITE streams are used for logs; flush each line so the file is
            // readable while the game is still running.
            if (this._mode == FileMode.WRITE && this._path != null) {
                fs_write(this._path, this._bytes);
            }
        }

        public function readBoolean():Boolean {
            return this.requireBytes().readBoolean();
        }

        public function readByte():int {
            return this.requireBytes().readByte();
        }

        public function readUnsignedByte():uint {
            return this.requireBytes().readUnsignedByte();
        }

        public function readShort():int {
            return this.requireBytes().readShort();
        }

        public function readUnsignedShort():uint {
            return this.requireBytes().readUnsignedShort();
        }

        public function readInt():int {
            return this.requireBytes().readInt();
        }

        public function readUnsignedInt():uint {
            return this.requireBytes().readUnsignedInt();
        }

        public function readFloat():Number {
            return this.requireBytes().readFloat();
        }

        public function readDouble():Number {
            return this.requireBytes().readDouble();
        }

        public function readMultiByte(length:uint, charSet:String):String {
            return this.requireBytes().readMultiByte(length, charSet);
        }

        public function readUTF():String {
            return this.requireBytes().readUTF();
        }

        public function readUTFBytes(length:uint):String {
            return this.requireBytes().readUTFBytes(length);
        }

        public function readBytes(bytes:ByteArray, offset:uint = 0, length:uint = 0):void {
            this.requireBytes().readBytes(bytes, offset, length);
        }

        public function readObject():* {
            return this.requireBytes().readObject();
        }

        public function writeBoolean(value:Boolean):void {
            this.requireBytes().writeBoolean(value);
            this.afterWrite();
        }

        public function writeByte(value:int):void {
            this.requireBytes().writeByte(value);
            this.afterWrite();
        }

        public function writeShort(value:int):void {
            this.requireBytes().writeShort(value);
            this.afterWrite();
        }

        public function writeInt(value:int):void {
            this.requireBytes().writeInt(value);
            this.afterWrite();
        }

        public function writeUnsignedInt(value:uint):void {
            this.requireBytes().writeUnsignedInt(value);
            this.afterWrite();
        }

        public function writeFloat(value:Number):void {
            this.requireBytes().writeFloat(value);
            this.afterWrite();
        }

        public function writeDouble(value:Number):void {
            this.requireBytes().writeDouble(value);
            this.afterWrite();
        }

        public function writeMultiByte(value:String, charSet:String):void {
            this.requireBytes().writeMultiByte(value, charSet);
            this.afterWrite();
        }

        public function writeUTF(value:String):void {
            this.requireBytes().writeUTF(value);
            this.afterWrite();
        }

        public function writeUTFBytes(value:String):void {
            this.requireBytes().writeUTFBytes(value);
            this.afterWrite();
        }

        public function writeBytes(bytes:ByteArray, offset:uint = 0, length:uint = 0):void {
            this.requireBytes().writeBytes(bytes, offset, length);
            this.afterWrite();
        }

        public function writeObject(object:*):void {
            this.requireBytes().writeObject(object);
            this.afterWrite();
        }
    }
}
