(() => {
  const send = body => window.webkit.messageHandlers.solidcraft.postMessage(body);
  window.addEventListener('error', e => send({kind:'error', text:String(e.message)}));
  window.addEventListener('unhandledrejection', e => send({kind:'error', text:String(e.reason)}));

  let fileRequest = null;
  window.showOpenFilePicker = () => new Promise((resolve, reject) => {
    if (fileRequest) { reject(new Error('A file picker is already open')); return; }
    fileRequest = {resolve, reject};
    send({kind:'pickFile'});
  });
  window.solidcraftPickedFile = (name, base64, error) => {
    const request = fileRequest; fileRequest = null;
    if (!request) return;
    if (!name) { request.reject(new DOMException(error || 'AbortError: Cancelled', error ? 'NotReadableError' : 'AbortError')); return; }
    const bytes = Uint8Array.from(atob(base64), c => c.charCodeAt(0));
    const file = new File([bytes], name);
    request.resolve([{name, getFile: async () => file}]);
  };

  // Rust exports through Blob downloads. Copy those bytes to native Documents.
  const blobs = new Map();
  const originalCreate = URL.createObjectURL.bind(URL);
  const originalRevoke = URL.revokeObjectURL.bind(URL);
  URL.createObjectURL = blob => { const url = originalCreate(blob); blobs.set(url, blob); return url; };
  URL.revokeObjectURL = url => { blobs.delete(url); originalRevoke(url); };
  const originalClick = HTMLAnchorElement.prototype.click;
  HTMLAnchorElement.prototype.click = function() {
    const blob = blobs.get(this.href);
    if (blob && this.download) {
      if (blob.size > 128 * 1024 * 1024) { send({kind:'error', text:'Export exceeds the prototype’s 128 MB limit.'}); return; }
      const reader = new FileReader();
      const name = this.download;
      reader.onload = () => send({kind:'export', name, base64:String(reader.result).split(',')[1]});
      reader.onerror = () => send({kind:'error', text:'Could not read export.'});
      reader.readAsDataURL(blob);
    } else originalClick.call(this);
  };

  document.addEventListener('DOMContentLoaded', () => {
    const canvas = document.getElementById('solidcraft_canvas');
    if (!canvas) return;
    canvas.style.touchAction = 'none';
    window.solidcraftIsModelPoint = p => {
      const rect = canvas.getBoundingClientRect();
      return window.wasmBindings?.touch_is_model?.(p.clientX-rect.left,p.clientY-rect.top) ?? true;
    };
    window.solidcraftAlignFace = p => {
      const rect = canvas.getBoundingClientRect();
      window.wasmBindings?.touch_align_face?.(p.clientX-rect.left,p.clientY-rect.top);
    };
    let lastReportedLayout = null;
    window.solidcraftReportLayout = () => setTimeout(() => {
      if (window.wasmBindings?.touch_layout) {
        const layout = JSON.parse(window.wasmBindings.touch_layout());
        if (window.solidcraftUITestProbe && window.wasmBindings.touch_project) {
          layout.qa_face = JSON.parse(window.wasmBindings.touch_project(20,10,8));
          if (layout.qa_face) layout.qa_face_is_model = window.wasmBindings.touch_is_model(...layout.qa_face);
          layout.qa_input = window.solidcraftInputDiagnostics;
        }
        const text = JSON.stringify(layout);
        if (text !== lastReportedLayout) {
          lastReportedLayout = text;
          send({kind:'layout',text});
        }
      }
    },150);
    // Debug UI tests exercise real palette commands through eframe's public DOM inputs.
    const wait = ms => new Promise(resolve => setTimeout(resolve,ms));
    window.solidcraftTestCommand = async line => {
        canvas.focus();
        const rect=canvas.getBoundingClientRect();
        const options={bubbles:true,cancelable:true,view:window,clientX:rect.left+240,
          clientY:rect.bottom-54,button:0,pointerType:'mouse',pointerId:100,isPrimary:true};
        canvas.dispatchEvent(new MouseEvent('mousemove',options));
        canvas.dispatchEvent(new PointerEvent('pointerdown',{...options,buttons:1}));
        canvas.dispatchEvent(new PointerEvent('pointerup',{...options,buttons:0}));
        await wait(150);
        const data=new DataTransfer(); data.setData('text/plain',line);
        canvas.dispatchEvent(new ClipboardEvent('paste',{bubbles:true,cancelable:true,clipboardData:data}));
        await wait(150);
        canvas.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',code:'Enter',bubbles:true,cancelable:true}));
        canvas.dispatchEvent(new KeyboardEvent('keyup',{key:'Enter',code:'Enter',bubbles:true,cancelable:true}));
        await wait(800);
    };
    window.solidcraftWorkflowCheck = async () => {
      const command = window.solidcraftTestCommand;
      await command('parameters.change {"name":"thickness","expression":"16 mm"}');
      await command('file.save {"path":"/opfs/iPad-Test.solidcraft"}');
      await command('doc.open {"path":"/opfs/iPad-Test.solidcraft"}');
      await command('file.save {"path":"/downloads/iPad-Test.json","format":"json"}');
      return true;
    };
    window.installSolidcraftInput(canvas, send);
    const timer=setInterval(async()=>{
      const loading=document.getElementById('solidcraft_loading');
      if (loading && loading.style.display !== 'none') return;
      clearInterval(timer);
      let storage='unverified';
      try {
        if (navigator.storage?.getDirectory) {
          const root=await navigator.storage.getDirectory();
          const f=await root.getFileHandle('prototype-storage-check.txt',{create:true});
          const w=await f.createWritable(); await w.write('SolidCraft storage OK'); await w.close();
          storage=(await (await f.getFile()).text()) === 'SolidCraft storage OK' ? 'OPFS read/write OK' : 'OPFS failed';
          await root.removeEntry('prototype-storage-check.txt');
        } else { localStorage.setItem('prototype-check','ok'); storage=localStorage.getItem('prototype-check') === 'ok' ? 'localStorage OK' : 'localStorage failed'; localStorage.removeItem('prototype-check'); }
      } catch(e) { storage='storage error: '+String(e); }
      send({kind:'ready',text:JSON.stringify({wasm:!!window.wasmBindings, canvas:[canvas.width,canvas.height], storage, renderer:'WebGL2', offline:true})});
      window.solidcraftReportLayout();
      setInterval(window.solidcraftReportLayout,500);
    },500);
  });
})();
