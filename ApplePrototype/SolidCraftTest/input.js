// Fingers navigate; Pencil and mouse retain distinct selection behavior.
window.installSolidcraftInput = (canvas, report) => {
  const fingers = new Map();
  let navigation = null, pencil = null, suspended = false, pendingOrbit = null, uiContact = null, lastTap = null;
  const ORBIT_INTENT_MS = 100;
  const ORBIT_SLOP = 6;
  // A two-finger twist must pass this angle before it rolls, so pans and pinches stay level.
  const TWIST_START = 0.1;
  // A finger on the UI: a quick swipe scrolls what is under it (with a short glide after it
  // lifts), a tap clicks, and a touch held this long before moving drags (sliders, handles).
  const UI_HOLD_MS = 200;
  const UI_SLOP = 8;
  const GLIDE_FRICTION = 0.95; // velocity kept per 16 ms
  let glide = null;
  const debug = {modelTaps:0,alignRequests:0,lastTapGap:0,lastRoute:''};
  window.solidcraftInputDiagnostics = debug;
  let orbitMoves = 0, panMoves = 0, zoomMoves = 0, rollMoves = 0, pencilStrokes = 0;
  let twist = {total:0,rolling:false};
  const stop = event => { event.preventDefault(); event.stopImmediatePropagation(); };
  const point = event => ({clientX:event.clientX, clientY:event.clientY});
  const emit = (type, p, mode, pressed) => {
    const primary = mode === 'pencil' || mode === 'ui';
    const options = {bubbles:true,cancelable:true,view:window,
      clientX:p.clientX,clientY:p.clientY,button:primary ? 0 : 1,
      buttons:pressed ? (primary ? 1 : 4) : 0,shiftKey:mode === 'orbit',
      pointerType:'mouse',pointerId:primary ? 10001 : 10002,isPrimary:true};
    canvas.dispatchEvent(type === 'mousemove' ? new MouseEvent(type,options) : new PointerEvent(type,options));
  };
  // Keep slight hand tremor from continually restarting egui's tooltip delay.
  // Both UIKit and WebKit pen hover use this one cursor path.
  let hoverPoint = null, hoverReported = false;
  window.solidcraftPencilHover = (x, y, ended) => {
    if (pencil || navigation || uiContact || fingers.size) return;
    if (ended) {
      hoverPoint = null;
      canvas.dispatchEvent(new MouseEvent('mouseleave',{bubbles:true,view:window}));
      return;
    }
    if (!hoverReported) {
      hoverReported = true;
      report({kind:'hover',text:'Pencil hover reached the input bridge.'});
    }
    if (hoverPoint && Math.hypot(x-hoverPoint.clientX,y-hoverPoint.clientY) < 5) return;
    hoverPoint = {clientX:x,clientY:y};
    emit('mousemove',hoverPoint,'pencil',false);
  };
  // Once fingers navigate, the engine uses the iPad gestures whatever mouse preset is chosen,
  // until a hardware mouse or trackpad is used. Events reach the engine a frame later, so the
  // state is not cleared when the fingers lift.
  const fingerNavigation = active => window.wasmBindings?.touch_set_finger_navigation?.(active);
  const releaseNavigation = () => {
    if (navigation) emit('pointerup',navigation.point,navigation.mode,false);
    navigation = null;
  };
  const measure = () => {
    const points = [...fingers.values()];
    if (points.length === 1) return {mode:'orbit',point:points[0],distance:null};
    if (points.length === 2) return {mode:'pan',
      point:{clientX:(points[0].clientX+points[1].clientX)/2,clientY:(points[0].clientY+points[1].clientY)/2},
      distance:Math.hypot(points[0].clientX-points[1].clientX,points[0].clientY-points[1].clientY),
      angle:Math.atan2(points[1].clientY-points[0].clientY,points[1].clientX-points[0].clientX)};
    return null;
  };
  const startNavigation = initial => {
    releaseNavigation();
    if (suspended || pencil) return;
    navigation = initial || measure();
    twist = {total:0,rolling:false};
    if (navigation) {
      fingerNavigation(true);
      emit('mousemove',navigation.point,navigation.mode,false);
      // eframe runs a frame on pointerup/down. Consume the cursor relocation
      // with the button released, so landing/centroid changes cannot rotate or pan.
      emit('pointerup',navigation.point,navigation.mode,false);
      emit('pointerdown',navigation.point,navigation.mode,true);
    }
  };
  const reportNavigation = () => report({kind:'navigation',text:
    `Touch navigation moves: ${orbitMoves+panMoves}; orbit: ${orbitMoves}; pan: ${panMoves}; pinch: ${zoomMoves}; rotate: ${rollMoves}; Pencil strokes: ${pencilStrokes}`});
  const modelPoint = p => window.solidcraftIsModelPoint ? window.solidcraftIsModelPoint(p) : true;
  const wheel = (p, dx, dy) => canvas.dispatchEvent(new WheelEvent('wheel',{bubbles:true,cancelable:true,
    clientX:p.clientX,clientY:p.clientY,deltaX:dx,deltaY:dy,deltaMode:0}));
  const stopGlide = () => {
    if (glide) cancelAnimationFrame(glide.frame);
    glide = null;
  };
  const startGlide = (at, vx, vy) => {
    stopGlide();
    if (Math.hypot(vx,vy) < 0.05) return;
    let last = performance.now();
    const step = now => {
      const dt = Math.min(now-last,50); last = now;
      const keep = Math.pow(GLIDE_FRICTION,dt/16);
      vx *= keep; vy *= keep;
      if (Math.hypot(vx,vy) < 0.02) { glide = null; return; }
      wheel(at,-vx*dt,-vy*dt);
      glide.frame = requestAnimationFrame(step);
    };
    glide = {frame:requestAnimationFrame(step)};
  };
  // A held UI touch becomes a press at its origin, then follows the finger as a drag.
  const pressUi = contact => {
    clearTimeout(contact.timer);
    contact.phase = 'drag';
    emit('mousemove',contact.origin,'ui',false);
    emit('pointerup',contact.origin,'ui',false);
    emit('pointerdown',contact.origin,'ui',true);
  };
  const releaseUi = () => {
    if (uiContact?.phase === 'drag') emit('pointerup',uiContact.point,'ui',false);
    if (uiContact) clearTimeout(uiContact.timer);
    uiContact = null;
  };
  // Menus and the command search close on a press outside them. A finger tap on the model is
  // navigation, not a press, so it also presses just off the canvas, where nothing is picked.
  const dismissOverlays = () => {
    const rect = canvas.getBoundingClientRect();
    const away = {clientX:rect.left-64,clientY:rect.top-64};
    emit('mousemove',away,'ui',false);
    emit('pointerdown',away,'ui',true);
    emit('pointerup',away,'ui',false);
  };

  for (const type of ['pointerdown','pointermove','pointerup','pointercancel']) {
    canvas.addEventListener(type,event => {
      // Translated events and the debug harness must reach eframe unchanged.
      if (!event.isTrusted) return;
      if (event.pointerType === 'mouse') {
        if (type === 'pointerdown' && !pencil) {
          hoverPoint = null;
          window.wasmBindings?.touch_set_pencil?.(false);
          fingerNavigation(false);
        }
        return;
      }
      stop(event);
      if (type === 'pointerdown') canvas.focus();
      const p = point(event);
      if (type === 'pointerdown') debug.lastRoute = event.pointerType + ':' + (modelPoint(p) ? 'model' : 'ui');
      if (event.pointerType === 'pen') {
        if (type === 'pointerdown') {
          window.wasmBindings?.touch_set_pencil?.(true);
          releaseUi(); stopGlide(); lastTap = null;
          releaseNavigation(); fingers.clear(); suspended = false; pendingOrbit = null;
          hoverPoint = null;
          pencil = {id:event.pointerId,point:p}; pencilStrokes++;
          emit('mousemove',p,'pencil',false);
          emit('pointerdown',p,'pencil',true);
        } else if (type === 'pointermove') {
          if (pencil && pencil.id !== event.pointerId) return;
          if (pencil) {
            pencil.point = p;
            emit('mousemove',p,'pencil',true);
          } else window.solidcraftPencilHover(p.clientX,p.clientY,false);
        } else if (pencil?.id === event.pointerId) {
          emit('pointerup',p,'pencil',false); pencil = null; reportNavigation();
        }
        return;
      }
      if (event.pointerType !== 'touch' || pencil) return;
      if (type === 'pointerdown') window.wasmBindings?.touch_set_pencil?.(false);
      if (type === 'pointerdown') stopGlide();
      if (uiContact) {
        if (uiContact.id === event.pointerId) {
          const c = uiContact;
          if (type === 'pointermove') {
            if (c.phase === 'pending' && Math.hypot(p.clientX-c.origin.clientX,p.clientY-c.origin.clientY) > UI_SLOP) {
              clearTimeout(c.timer);
              c.phase = 'scroll';
              c.last = c.origin; c.lastTime = event.timeStamp; c.vx = 0; c.vy = 0;
            }
            if (c.phase === 'scroll') {
              const dx = p.clientX-c.last.clientX, dy = p.clientY-c.last.clientY;
              const dt = Math.max(event.timeStamp-c.lastTime,1);
              // The content follows the finger: moving up reveals what is below.
              wheel(c.origin,-dx,-dy);
              c.vx = 0.7*c.vx+0.3*dx/dt; c.vy = 0.7*c.vy+0.3*dy/dt;
              c.last = p; c.lastTime = event.timeStamp;
            } else if (c.phase === 'drag') emit('mousemove',p,'ui',true);
            c.point = p;
          } else if (type === 'pointerup' || type === 'pointercancel') {
            if (c.phase === 'pending' && type === 'pointerup') {
              // A tap: press and let go where it landed.
              pressUi(c);
              emit('pointerup',c.origin,'ui',false);
              c.phase = 'done';
            } else if (c.phase === 'scroll' && type === 'pointerup' && event.timeStamp-c.lastTime < 80) {
              startGlide(c.origin,c.vx,c.vy);
            }
            releaseUi();
            if (window.solidcraftReportLayout) window.solidcraftReportLayout();
          }
        }
        return;
      }
      if (type === 'pointerdown' && !modelPoint(p)) {
        releaseNavigation(); fingers.clear(); pendingOrbit = null; lastTap = null; suspended = false;
        const contact = {id:event.pointerId,point:p,origin:p,phase:'pending'};
        contact.timer = setTimeout(() => { if (uiContact === contact && contact.phase === 'pending') pressUi(contact); },UI_HOLD_MS);
        uiContact = contact;
        // Hover first, so what is under the finger highlights and a swipe scrolls it.
        emit('mousemove',p,'ui',false);
        return;
      }
      if (type === 'pointerdown') {
        fingers.set(event.pointerId,p);
        if (fingers.size > 2) suspended = true; // Ignore additional contacts/palms.
        if (fingers.size === 1 && !suspended) {
          // Give a second finger time to land, and ignore resting-finger jitter.
          pendingOrbit = {origin:p,startedAt:event.timeStamp};
        } else {
          pendingOrbit = null;
          lastTap = null;
          startNavigation();
        }
      } else if (type === 'pointermove' && fingers.has(event.pointerId)) {
        fingers.set(event.pointerId,p);
        const next = measure();
        if (pendingOrbit && fingers.size === 1 && !suspended) {
          const distance = Math.hypot(p.clientX-pendingOrbit.origin.clientX,p.clientY-pendingOrbit.origin.clientY);
          if (event.timeStamp-pendingOrbit.startedAt < ORBIT_INTENT_MS || distance < ORBIT_SLOP) return;
          startNavigation({mode:'orbit',point:pendingOrbit.origin,distance:null});
          pendingOrbit = null;
          lastTap = null;
        }
        if (!navigation || !next || suspended) return;
        emit('mousemove',next.point,next.mode,true);
        if (next.mode === 'orbit') orbitMoves++; else panMoves++;
        if (next.mode === 'pan' && navigation.distance > 0 && next.distance > 0) {
          const ratio = next.distance/navigation.distance;
          if (Math.abs(Math.log(ratio)) > 0.0001) {
            canvas.dispatchEvent(new WheelEvent('wheel',{bubbles:true,cancelable:true,
              clientX:next.point.clientX,clientY:next.point.clientY,deltaY:-Math.log(ratio)/0.0015,deltaMode:0}));
            zoomMoves++;
          }
        }
        if (next.mode === 'pan' && navigation.angle !== undefined) {
          // Screen angles grow clockwise, as a positive camera roll turns the picture.
          let turn = next.angle-navigation.angle;
          turn = Math.atan2(Math.sin(turn),Math.cos(turn));
          twist.total += turn;
          if (!twist.rolling && Math.abs(twist.total) > TWIST_START) {
            twist.rolling = true;
            turn = twist.total;
          }
          if (twist.rolling && Math.abs(turn) > 0.0001) {
            window.wasmBindings?.touch_roll?.(turn);
            rollMoves++;
          }
        }
        navigation = next;
      } else if ((type === 'pointerup' || type === 'pointercancel') && fingers.has(event.pointerId)) {
        const tap = type === 'pointerup' && fingers.size === 1 && pendingOrbit
          && event.timeStamp-pendingOrbit.startedAt < 350
          && Math.hypot(p.clientX-pendingOrbit.origin.clientX,p.clientY-pendingOrbit.origin.clientY) < ORBIT_SLOP;
        if (tap) {
          debug.modelTaps++;
          dismissOverlays();
          if (lastTap) debug.lastTapGap = event.timeStamp-lastTap.time;
          if (lastTap && event.timeStamp-lastTap.time < 350 && Math.hypot(p.clientX-lastTap.point.clientX,p.clientY-lastTap.point.clientY) < 20) {
            debug.alignRequests++;
            if (window.solidcraftAlignFace) window.solidcraftAlignFace(p);
            lastTap = null;
          } else lastTap = {point:p,time:event.timeStamp};
        } else lastTap = null;
        pendingOrbit = null;
        releaseNavigation(); fingers.delete(event.pointerId);
        // Lifting one finger after panning must not suddenly orbit the model.
        suspended = fingers.size > 0;
        if (!fingers.size) {
          suspended = false; reportNavigation();
          if (window.solidcraftReportLayout) window.solidcraftReportLayout();
        }
      }
    },{capture:true,passive:false});
  }
  canvas.addEventListener('wheel',event => {
    if (event.isTrusted) fingerNavigation(false);
  },{capture:true,passive:true});
  canvas.addEventListener('pointerleave',event => {
    if (event.isTrusted && event.pointerType === 'pen' && !pencil) {
      window.solidcraftPencilHover(0,0,true);
    }
  },{capture:true});
  // eframe also subscribes to legacy touch and compatibility mouse events.
  for (const type of ['touchstart','touchmove','touchend','touchcancel','gesturestart','gesturechange','gestureend']) {
    canvas.addEventListener(type,stop,{capture:true,passive:false});
  }
  canvas.addEventListener('mousemove',event => {
    if (event.isTrusted && (navigation || pencil || uiContact || fingers.size || hoverPoint)) stop(event);
  },{capture:true,passive:false});
  canvas.addEventListener('mouseleave',event => {
    if (event.isTrusted && hoverPoint) stop(event);
  },{capture:true,passive:false});
  window.addEventListener('blur',() => {
    releaseNavigation();
    if (pencil) emit('pointerup',pencil.point,'pencil',false);
    releaseUi(); stopGlide();
    hoverPoint = null;
    pencil = null; fingers.clear(); suspended = false; pendingOrbit = null; lastTap = null;
  });
};
