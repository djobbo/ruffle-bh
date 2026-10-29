package flash.external {
    import __ruffle__.get_command_line;
    import __ruffle__.log_warn;
    import __ruffle__.swz_decode;

    import flash.events.EventDispatcher;
    import flash.events.StatusEvent;
    import flash.utils.ByteArray;

    /**
     * AIR native extension bridge.
     *
     * Ruffle cannot load ANEs, so unsupported extension IDs resolve to `null`
     * (which is what AIR does when an extension is missing) and let the game
     * take its own graceful-degradation path. The extensions Brawlhalla needs
     * for a browser build are emulated in-process:
     *
     * - `RawData` decrypts `.swz` containers (the real ANE is where Brawlhalla's
     *   keyed container lives).
     * - `SteamAir` reports a minimal logged-in Steam client so the retail boot
     *   path can proceed when the game is not in standalone mode.
     */
    [API("669")]
    public final class ExtensionContext extends EventDispatcher {
        private var _extensionID:String = null;
        private var _rawDataKey:uint = 0;
        private var _rawDataEntries:Array = null;
        private var _rawDataIndex:int = 0;
        private var _steamInitialized:Boolean = false;
        private var _steamTicketSent:Boolean = false;

        public function ExtensionContext() {
            super();
        }

        public static function createExtensionContext(extensionID:String, contextType:String):ExtensionContext {
            switch (extensionID) {
                case "RawData":
                case "SteamAir":
                case "DesktopExtension":
                case "WindowsExtensionWrapper":
                    var context:ExtensionContext = new ExtensionContext();
                    context._extensionID = extensionID;
                    return context;
                default:
                    // Match AIR: unknown/missing extensions yield null. Callers
                    // guard on this and skip telemetry, raw keyboard, etc.
                    return null;
            }
        }

        public function call(functionName:String, ...rest):Object {
            switch (this._extensionID) {
                case "RawData":
                    return this.callRawData(functionName, rest);
                case "SteamAir":
                    return this.callSteam(functionName, rest);
                case "DesktopExtension":
                    if (functionName == "DesktopExtension_GetCommandLine") {
                        return get_command_line();
                    }
                    return null;
                case "WindowsExtensionWrapper":
                    if (functionName == "as_getRefreshRate") {
                        return 60;
                    }
                    return null;
                default:
                    return null;
            }
        }

        private function callRawData(functionName:String, args:Array):Object {
            switch (functionName) {
                case "RawData_Init":
                    this._rawDataKey = args.length > 0 ? uint(args[0]) : 0;
                    this._rawDataEntries = null;
                    this._rawDataIndex = 0;
                    return true;
                case "RawData_SetData":
                    this._rawDataEntries = null;
                    this._rawDataIndex = 0;
                    if (args.length == 0 || args[0] == null) {
                        return false;
                    }
                    this._rawDataEntries = swz_decode(args[0] as ByteArray, this._rawDataKey);
                    log_warn("[RawData] SetData key=" + this._rawDataKey + " entries=" + (this._rawDataEntries != null ? this._rawDataEntries.length : -1));
                    return this._rawDataEntries != null;
                case "RawData_GetData":
                    if (this._rawDataEntries == null || this._rawDataIndex >= this._rawDataEntries.length) {
                        return null;
                    }
                    var data:String = this._rawDataEntries[this._rawDataIndex++];
                    log_warn("[RawData] entry " + this._rawDataIndex + " len=" + data.length + " head=" + data.substring(0, 40));
                    return data;
                default:
                    return null;
            }
        }

        private function callSteam(functionName:String, args:Array):Object {
            switch (functionName) {
                case "SteamAir_Init":
                    this._steamInitialized = true;
                    return true;
                case "SteamAir_RestartAppIfNecessary":
                    return false;
                case "SteamAir_IsSubscribedApp":
                    return true;
                case "SteamAir_RunCallbacks":
                    if (!this._steamTicketSent) {
                        this._steamTicketSent = true;
                        // The real ANE reports the ticket asynchronously. By
                        // the first RunCallbacks the game has installed its
                        // response listener.
                        this.dispatchEvent(new StatusEvent(
                            StatusEvent.STATUS, false, false, "1", "1"
                        ));
                    }
                    return null;
                case "SteamAir_GetPersonaName":
                    return "Player";
                case "SteamAir_GetPersonaID":
                case "SteamAir_GetSteamID":
                    return "76561198000000000";
                case "SteamAir_GetOrderID":
                    return "0";
                case "SteamAir_GetLanguage":
                    return "english";
                case "SteamAir_IsOverlayEnabled":
                    return false;
                case "SteamAir_IsSteamDeck":
                    return false;
                case "SteamAir_IsSteamInBigPicture":
                    return false;
                case "SteamAir_GetSteamOrder":
                    return [];
                case "SteamAir_GetEncryptedAppTicket":
                    var ticket:Vector.<int> = new Vector.<int>();
                    ticket.push(1);
                    return ticket;
                case "SteamAir_GetIcon":
                    return null;
                case "SteamAir_TriggerAchievement":
                case "SteamAir_StoreStats":
                case "SteamAir_ActivateGameOverlayToUser":
                    return null;
                default:
                    return null;
            }
        }

        public function dispose():void {
            this._rawDataEntries = null;
            this._rawDataIndex = 0;
        }
    }
}
