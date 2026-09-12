var bc = WScript.CreateObject('BridgeComposer.Object');
bc.Noui = true;
if (!bc.Open(WScript.Arguments(0))) { WScript.Echo('open failed'); WScript.Quit(1); }
bc.SaveAsPdf(WScript.Arguments(1));
