use crate::data::{Assets, f32_of};
use crate::course::Course;
fn geo(c:char)->bool {crate::envc::var("JPRACE_GEO_F32").map(|v|v.contains(c)).unwrap_or(false)}
type V3 = [f32; 3];
type Q = [f32; 4];
fn qmul(a: Q, b: Q) -> Q {
    let [ax,ay,az,aw]=a; let [bx,by,bz,bw]=b;
    [aw*bx+ax*bw+ay*bz-az*by, aw*by+ay*bw+az*bx-ax*bz,
     aw*bz+az*bw+ax*by-ay*bx, aw*bw-ax*bx-ay*by-az*bz]
}
fn qlerp_old(a: Q, b: Q, t: f32) -> Q {
    let t=t.clamp(0.0,1.0);
    let dot=a[0]*b[0]+a[1]*b[1]+a[2]*b[2]+a[3]*b[3];
    let sign=if dot<0.0 {-1.0} else {1.0};
    let mut q=std::array::from_fn::<_,4,_>(|i|a[i]+t*(sign*b[i]-a[i]));
    let n=(q[0]*q[0]+q[1]*q[1]+q[2]*q[2]+q[3]*q[3]).sqrt();
    for v in &mut q { *v/=n; } q
}
fn dot4(a: Q, b: Q) -> f32 {
    static M:std::sync::OnceLock<u8>=std::sync::OnceLock::new();
    let p=[a[0]*b[0],a[1]*b[1],a[2]*b[2],a[3]*b[3]];
    match *M.get_or_init(||match crate::envc::var("JPRACE_QDOT").as_deref() {Ok("pair")=>1,Ok("pair2")=>2,_=>0}) {
        1=>(p[0]+p[1])+(p[2]+p[3]), 2=>(p[0]+p[2])+(p[1]+p[3]), _=>p[0]+p[1]+p[2]+p[3]}
}
fn qlerp(a: Q, b: Q, t: f32) -> Q {
    let t1=1.0-t;
    let dot=dot4(a,b);
    let r:Q=if dot>=0.0 {std::array::from_fn(|i|t1*a[i]+t*b[i])} else {std::array::from_fn(|i|t1*a[i]-t*b[i])};
    static N:std::sync::OnceLock<bool>=std::sync::OnceLock::new();
    if *N.get_or_init(||crate::envc::var("JPRACE_QNORM").as_deref()==Ok("div")) {let len=dot4(r,r).sqrt();return std::array::from_fn(|i|r[i]/len);}
    let inv=1.0/dot4(r,r).sqrt();
    std::array::from_fn(|i|r[i]*inv)
}
fn qmul_numerics(a: Q, b: Q) -> Q {
    let [ax,ay,az,aw]=a; let [bx,by,bz,bw]=b;
    let (cx,cy,cz)=(ay*bz-az*by, az*bx-ax*bz, ax*by-ay*bx);
    let dot=ax*bx+ay*by+az*bz;
    [ax*bw+bx*aw+cx, ay*bw+by*aw+cy, az*bw+bz*aw+cz, aw*bw-dot]
}
pub fn geo_old()->bool {static O:std::sync::OnceLock<bool>=std::sync::OnceLock::new();*O.get_or_init(||crate::envc::var_os("JPRACE_GEO_OLD").is_some())}
fn geo_t()->&'static str {static S:std::sync::OnceLock<String>=std::sync::OnceLock::new();S.get_or_init(||crate::envc::var("JPRACE_GEO_T").unwrap_or_default()).as_str()}
pub struct Geometry {
    keys: Vec<V3>, rotations: Vec<Q>, distance: f32, fix: Q,
    goal: V3, goal_rot: Q, goal_dir: V3, pub distance_to_world: f32,
    overrun: Option<(Vec<V3>,Vec<Q>,f32)>,
}
type Keys=(Vec<V3>,Vec<Q>);
fn lane_keys(a:&Assets,name:&str)->Result<Keys,String> {
    let k=&a.lane_asset(name)["key"];
    let x=k["valueX"].as_array().ok_or("lane x keys")?;
    let z=k["valueZ"].as_array().ok_or("lane z keys")?;
    let y=k["valueY"].as_array().filter(|v| !v.is_empty());
    if x.is_empty() || z.len()!=x.len() || y.is_some_and(|y|y.len()!=x.len()) {
        return Err("inconsistent lane position keys".into());
    }
    let keys=x.iter().enumerate().map(|(i,x)|[f32_of(x),y.map_or(0.0,|y|f32_of(&y[i])),f32_of(&z[i])]).collect();
    let rotations=k["rotation"].as_array().ok_or("rotation keys")?.iter()
        .map(|r|[f32_of(&r["x"]),f32_of(&r["y"]),f32_of(&r["z"]),f32_of(&r["w"])]).collect::<Vec<_>>();
    if rotations.is_empty() { return Err("empty rotation keys".into()); }
    Ok((keys,rotations))
}
impl Geometry {
    pub fn load(a: &Assets, c: &Course) -> Result<Self,String> {
        let name=a.course_set(c.id)["normal_lane_asset"].as_str().ok_or("normal lane asset")?;
        let (keys,rotations)=lane_keys(a,name)?;
        let overrun=(|| {
            let (dir,file)=name.rsplit_once('/')?;let p=file.split('_').collect::<Vec<_>>();
            if p.len()!=8 {return None;}
            let o=format!("{dir}/{}_{}_{}_01_0000_{}_0_{}",p[0],p[1],p[2],p[5],p[7]);
            if a.lane_asset(&o).is_null() {return None;}
            let d=a.lane_asset(&o)["Distance"].as_f64()? as f32;
            lane_keys(a,&o).ok().map(|(k,r)|(k,r,d))
        })();
        let h=(if c.turn==1 {180.0f32} else {0.0}) * 0.01745329238474369 * 0.5;
        let mut g=Self { keys, rotations, distance:c.distance, fix:[0.0,h.sin(),0.0,h.cos()],
            goal:[0.0;3],goal_rot:[0.0;4],goal_dir:[0.0;3],distance_to_world:0.0,overrun:None };
        let (p,_) = g.transform(c.distance-10.0);
        (g.goal,g.goal_rot)=g.transform(c.distance);
        g.goal_dir=std::array::from_fn(|i|(g.goal[i]-p[i])*0.1);
        g.overrun=overrun;
        let a=g.world(0.0,0.0);let b=g.world(1.0,0.0);
        let [dx,dy,dz]=std::array::from_fn(|i|a[i]-b[i]);
        g.distance_to_world=if geo('d') || !geo_old() {(dx*dx+dy*dy+dz*dz).sqrt()} else {((dy as f64)*(dy as f64)+(dx as f64)*(dx as f64)+(dz as f64)*(dz as f64)).sqrt() as f32};
        match crate::envc::var("JPRACE_DTW_MODE").as_deref() {
            Ok("2d")=>g.distance_to_world=(dx*dx+dz*dz).sqrt(),
            Ok("f64")=>g.distance_to_world=((dx as f64)*(dx as f64)+(dy as f64)*(dy as f64)+(dz as f64)*(dz as f64)).sqrt() as f32,
            Ok("yxz")=>g.distance_to_world=(dy*dy+dx*dx+dz*dz).sqrt(),
            _=>{}
        }
        if let Some(k)=crate::envc::var("JPRACE_DTW_ULP").ok().and_then(|x|x.parse::<i32>().ok()) {g.distance_to_world=f32::from_bits((g.distance_to_world.to_bits() as i32+k) as u32);}
        if crate::envc::var_os("JPRACE_LOG_DTW").is_some() {eprintln!("dtw {:08x} {:e} d=({:e},{:e},{:e})",g.distance_to_world.to_bits(),g.distance_to_world,dx,dy,dz);}
        Ok(g)
    }
    fn transform(&self,d:f32)->(V3,Q) {
        if d>self.distance && let Some((keys,rots,od))=&self.overrun {
            let r=(d-self.distance)%od;
            let t=match crate::envc::var("JPRACE_OVR_T").as_deref() {
                Ok("f64")=>(((d as f64-self.distance as f64)%(*od as f64))/(*od as f64)*1000.0) as f32,
                Ok("m")=>r*1000.0/ *od,
                Ok("inv")=>r*(1000.0/ *od),
                Ok("nomod")=>(d-self.distance)/ *od*1000.0,
                _=>if geo_old() {((r as f64)/(*od as f64)*1000.0) as f32} else {r/ *od*1000.0}};
            return self.sample(keys,rots,t);
        }
        if d>self.distance {
            return (std::array::from_fn(|i|self.goal_dir[i]*(d-self.distance)+self.goal[i]), self.goal_rot);
        }
        if !geo_old() {
            match geo_t() {
                "f64"=>return self.sample(&self.keys,&self.rotations,((d as f64)/(self.distance as f64)*1000.0) as f32),
                "f64f"=>{let t=(d as f64)/(self.distance as f64)*1000.0;let i=t as usize;return self.sample_if(&self.keys,&self.rotations,i,(t-i as f64) as f32);},
                "m"=>return self.sample(&self.keys,&self.rotations,d*1000.0/self.distance),
                "inv"=>return self.sample(&self.keys,&self.rotations,d*(1000.0/self.distance)),
                "div"=>return self.sample(&self.keys,&self.rotations,d/(self.distance/1000.0)),
                _=>{}
            }
        }
        let t=if geo('t') || !geo_old() {d/self.distance*1000.0} else {((d as f64)/(self.distance as f64)*1000.0) as f32};
        self.sample(&self.keys,&self.rotations,t)
    }
    fn sample_if(&self,keys:&[V3],rotations:&[Q],i:usize,f:f32)->(V3,Q) {
        let p=if i<keys.len()-1 {std::array::from_fn(|j|keys[i][j]*(1.0-f)+keys[i+1][j]*f)} else {keys[keys.len()-1]};
        let r=if i<rotations.len()-1 {qlerp(rotations[i],rotations[i+1],f)} else {rotations[rotations.len()-1]};
        (p,qmul_numerics(r,self.fix))
    }
    fn sample(&self,keys:&[V3],rotations:&[Q],t:f32)->(V3,Q) {
        let i=t as usize;
        if geo_old() {
            let p=if i<keys.len()-1 {
                let f=(t-i as f32).clamp(0.0,1.0);
                if geo('l') {std::array::from_fn(|j|keys[i][j]+(keys[i+1][j]-keys[i][j])*f)} else {
                std::array::from_fn(|j|(((keys[i+1][j] as f64)-(keys[i][j] as f64))*(f as f64)+(keys[i][j] as f64)) as f32)}
            } else {keys[keys.len()-1]};
            let r=if i<rotations.len()-1 {qlerp_old(rotations[i],rotations[i+1],t-i as f32)}
                else {rotations[rotations.len()-1]};
            return (p,qmul(r,self.fix));
        }
        let f=t-i as f32;
        let fm=crate::envc::var("JPRACE_GEO_FMA").unwrap_or_default();
        let p=if i<keys.len()-1 {
            if fm.contains('l') {std::array::from_fn(|j|keys[i][j].mul_add(1.0-f,keys[i+1][j]*f))}
            else if fm.contains('L') {std::array::from_fn(|j|(keys[i+1][j]-keys[i][j]).mul_add(f,keys[i][j]))}
            else {std::array::from_fn(|j|keys[i][j]*(1.0-f)+keys[i+1][j]*f)}
        } else {keys[keys.len()-1]};
        let r=if i<rotations.len()-1 {qlerp(rotations[i],rotations[i+1],f)} else {rotations[rotations.len()-1]};
        (p,qmul_numerics(r,self.fix))
    }
    pub fn world(&self,d:f32,lane:f32)->V3 {
        let (o,[x,y,z,w])=self.transform(d);
        let l=lane*11.25;
        let [x2,y2,z2]=[x+x,y+y,z+z];let yy=y2*y;let zz=z2*z;
        let (l64,f)=(l as f64,|a:f32,b:f32,sgn:f64|a as f64+sgn*b as f64);
        if !geo_old() {
            if crate::envc::var("JPRACE_GEO_FMA").unwrap_or_default().contains('w') {return [l.mul_add((1.0-yy)-zz,o[0]),l.mul_add(x*y2+w*z2,o[1]),l.mul_add(x*z2-w*y2,o[2])];}
            match crate::envc::var("JPRACE_GEO_MAT").as_deref() {
                Ok("1")=>{let (xx,yy,zz,xy,xz,yw,zw)=(x*x,y*y,z*z,x*y,x*z,y*w,z*w);return [l*(1.0-2.0*(yy+zz))+o[0],l*(2.0*(xy+zw))+o[1],l*(2.0*(xz-yw))+o[2]];}
                Ok("2")=>{let (yy,zz,xy,xz,yw,zw)=(y*y,z*z,x*y,x*z,y*w,z*w);return [l*((1.0-2.0*yy)-2.0*zz)+o[0],l*(2.0*xy+2.0*zw)+o[1],l*(2.0*xz-2.0*yw)+o[2]];}
                _=>{}
            }
            if crate::envc::var("JPRACE_GEO_UNITY").as_deref()==Ok("1") {return [(1.0-(yy+zz))*l+o[0],(x*y2+w*z2)*l+o[1],(x*z2-w*y2)*l+o[2]];}
            return [l*((1.0-yy)-zz)+o[0],l*(x*y2+w*z2)+o[1],l*(x*z2-w*y2)+o[2]];}
        if geo('w') {return [(1.0-(yy+zz))*l+o[0],(x2*y+z2*w)*l+o[1],(x2*z-y2*w)*l+o[2]];}
        [((1.0-f(yy,zz,1.0))*l64) as f32+o[0],(f(x2*y,z2*w,1.0)*l64) as f32+o[1],(f(x2*z,y2*w,-1.0)*l64) as f32+o[2]]
    }
    pub fn world64(&self,d:f32,lane:f32)->[f64;3] {
        if d>self.distance {let w=self.world(d,lane);return [w[0] as f64,w[1] as f64,w[2] as f64];}
        let t=d as f64/self.distance as f64*1000.0;
        let (keys,rots)=(&self.keys,&self.rotations);
        let i=t as usize;
        let p:[f64;3]=if i<keys.len()-1 {let f=(t-i as f64).clamp(0.0,1.0);std::array::from_fn(|j|(keys[i+1][j] as f64-keys[i][j] as f64)*f+keys[i][j] as f64)} else {std::array::from_fn(|j|keys[keys.len()-1][j] as f64)};
        let q:[f64;4]=if i<rots.len()-1 {
            let (a,b)=(rots[i],rots[i+1]);let tt=(t-i as f64).clamp(0.0,1.0);
            let dot=(0..4).map(|k|a[k] as f64*b[k] as f64).sum::<f64>();let sg=if dot<0.0 {-1.0} else {1.0};
            let mut q:[f64;4]=std::array::from_fn(|k|a[k] as f64+tt*(sg*b[k] as f64-a[k] as f64));
            let n=(q.iter().map(|v|v*v).sum::<f64>()).sqrt();for v in &mut q {*v/=n;} q
        } else {std::array::from_fn(|k|rots[rots.len()-1][k] as f64)};
        let f=self.fix;let [ax,ay,az,aw]=q;let [bx,by,bz,bw]=[f[0] as f64,f[1] as f64,f[2] as f64,f[3] as f64];
        let [x,y,z,w]=[aw*bx+ax*bw+ay*bz-az*by, aw*by+ay*bw+az*bx-ax*bz, aw*bz+az*bw+ax*by-ay*bx, aw*bw-ax*bx-ay*by-az*bz];
        let l=lane as f64*11.25;
        [(1.0-(2.0*y*y+2.0*z*z))*l+p[0],(2.0*x*y+2.0*z*w)*l+p[1],(2.0*x*z-2.0*y*w)*l+p[2]]
    }
    pub fn raw(&self,d:f32)->String {
        let t64=((d as f64)/(self.distance as f64)*1000.0) as f32;let t=d/self.distance*1000.0;
        let i=t as usize;let j=(i+1).min(self.keys.len()-1);
        let im=i.saturating_sub(1);let j2=(i+2).min(self.keys.len()-1);
        format!("t={:e} t64={:e} i={} k0={:?} k1={:?} r0={:?} r1={:?} fix={:?} km={:?} k2={:?}",t,t64,i,self.keys[i],self.keys[j],self.rotations[i],self.rotations[(i+1).min(self.rotations.len()-1)],self.fix,self.keys[im],self.keys[j2])
    }
}
