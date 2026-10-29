(() => {
  const canvas = document.querySelector('#globe-points');
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
  let width = 0, height = 0, dpr = 1, longitude = -.45, tilt = -.12, pointerX = 0, pointerY = 0, hovering = false, last = performance.now();

  // Distribute exactly 1,000 particles evenly across a complete sphere.
  const particles=[];
  const particleCount=1000;
  const goldenAngle=Math.PI*(3-Math.sqrt(5));
  for(let i=0;i<particleCount;i++){
    const y=1-2*(i+.5)/particleCount;
    const radius=Math.sqrt(1-y*y);
    const theta=goldenAngle*i;
    particles.push({x:Math.cos(theta)*radius,y,z:Math.sin(theta)*radius,phase:(i*2.399)%(Math.PI*2),size:.72+((i*37)%13)/22});
  }

  const resize = () => {
    const box = canvas.getBoundingClientRect();
    dpr = Math.min(devicePixelRatio || 1, 2); width = box.width; height = box.height;
    canvas.width = Math.round(width*dpr);canvas.height = Math.round(height*dpr);ctx.setTransform(dpr,0,0,dpr,0,0);
  };
  new ResizeObserver(resize).observe(canvas); resize();
  const rotatePoint=(point,time=0)=>{const yaw=longitude,pitch=tilt,wave=1+.026*Math.sin(point.phase+time*.0024),x=point.x*wave,y=point.y*wave,z=point.z*wave,xx=x*Math.cos(yaw)+z*Math.sin(yaw),zz=-x*Math.sin(yaw)+z*Math.cos(yaw);return{x:xx,y:y*Math.cos(pitch)-zz*Math.sin(pitch),z:y*Math.sin(pitch)+zz*Math.cos(pitch)}};
  const project=(lon,lat)=>{const p=lat*Math.PI/180,l=lon*Math.PI/180+longitude,x=Math.cos(p)*Math.sin(l),y=Math.sin(p),z=Math.cos(p)*Math.cos(l);return{x,y:y*Math.cos(tilt)-z*Math.sin(tilt),z:y*Math.sin(tilt)+z*Math.cos(tilt)}};
  const draw = now => {
    const dt=Math.min((now-last)/1000,.05);last=now;if(!reduced)longitude+=dt*.12;tilt=-.12+pointerY*.08;
    const cx=width/2,cy=height/2,R=Math.min(width,height)*.385;ctx.clearRect(0,0,width,height);
    const dots=[];
    for(const point of particles){let q=rotatePoint(point,now);let x=cx+R*q.x,y=cy-R*q.y;const distance=Math.hypot(x-pointerX,y-pointerY),influence=hovering?Math.max(0,1-distance/115):0;if(influence){const wobble=Math.sin(now*.018+point.phase*2)*influence*6.5;x+=wobble;y+=Math.cos(now*.016+point.phase)*influence*6.5;q={...q,x:(x-cx)/R,y:-(y-cy)/R}}const depth=.3+.7*Math.max(0,q.z);dots.push({x,y,z:q.z,r:point.size*(.9+.75*Math.max(0,q.z))*(R/105),depth,front:q.z>0,latitude:q.y,influence})}
    dots.sort((a,b)=>a.z-b.z);
    for(const d of dots){ctx.beginPath();ctx.arc(d.x,d.y,d.r,0,Math.PI*2);ctx.fillStyle=`rgba(255,255,255,${d.front?.72+.28*d.depth:.08+.17*(d.z+1)})`;ctx.shadowColor='rgba(255,255,255,.92)';ctx.shadowBlur=d.front&&d.depth>.7?2.8:0;ctx.fill()}
    ctx.shadowBlur=0;
    // A perspective orbit sweeps around the sphere, with a bright moving point.
    ctx.beginPath();let start=true;for(let i=0;i<=150;i++){const t=i/150*Math.PI*2,q=project(-180+i*2.4,14+29*Math.sin(t));if(q.z>.02){const x=cx+R*1.035*q.x,y=cy-R*1.035*q.y;if(start){ctx.moveTo(x,y);start=false}else ctx.lineTo(x,y)}else start=true}ctx.strokeStyle='rgba(241,43,40,.58)';ctx.lineWidth=1.3;ctx.setLineDash([3,8]);ctx.lineDashOffset=-now*.012;ctx.stroke();ctx.setLineDash([]);
    requestAnimationFrame(draw);
  };
  canvas.addEventListener('pointermove',e=>{const r=canvas.getBoundingClientRect(),nx=e.clientX-r.left,ny=e.clientY-r.top;pointerX=nx;pointerY=(ny-r.height/2)/(r.height/2)});
  canvas.addEventListener('pointerenter',e=>{const r=canvas.getBoundingClientRect();pointerX=e.clientX-r.left;hovering=true});
  canvas.addEventListener('pointerleave',()=>{pointerY=0;hovering=false});requestAnimationFrame(draw);
})();
