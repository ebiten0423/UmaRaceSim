use crate::data::{Assets, f32_of};
use crate::course::Course;
type V3 = [f32; 3];
type Q = [f32; 4];
fn qmul(a: Q, b: Q) -> Q {
    let [ax,ay,az,aw]=a; let [bx,by,bz,bw]=b;
    [aw*bx+ax*bw+ay*bz-az*by, aw*by+ay*bw+az*bx-ax*bz,
     aw*bz+az*bw+ax*by-ay*bx, aw*bw-ax*bx-ay*by-az*bz]
}
fn qlerp(a: Q, b: Q, t: f32) -> Q {
    let t=t.clamp(0.0,1.0);
    let dot=a[0]*b[0]+a[1]*b[1]+a[2]*b[2]+a[3]*b[3];
    let sign=if dot<0.0 {-1.0} else {1.0};
    let mut q=std::array::from_fn::<_,4,_>(|i|a[i]+t*(sign*b[i]-a[i]));
    let n=(q[0]*q[0]+q[1]*q[1]+q[2]*q[2]+q[3]*q[3]).sqrt();
    for v in &mut q { *v/=n; } q
}
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
        g.distance_to_world=((dy as f64)*(dy as f64)+(dx as f64)*(dx as f64)+(dz as f64)*(dz as f64)).sqrt() as f32;
        Ok(g)
    }
    fn transform(&self,d:f32)->(V3,Q) {
        if d>self.distance && let Some((keys,rots,od))=&self.overrun {
            let r=(d-self.distance)%od;
            let t=((r as f64)/(*od as f64)*1000.0) as f32;
            return self.sample(keys,rots,t);
        }
        if d>self.distance {
            return (std::array::from_fn(|i|self.goal_dir[i]*(d-self.distance)+self.goal[i]), self.goal_rot);
        }
        let t=((d as f64)/(self.distance as f64)*1000.0) as f32;
        self.sample(&self.keys,&self.rotations,t)
    }
    fn sample(&self,keys:&[V3],rotations:&[Q],t:f32)->(V3,Q) {
        let i=t as usize;
        let p=if i<keys.len()-1 {
            let f=(t-i as f32).clamp(0.0,1.0);
            std::array::from_fn(|j|(((keys[i+1][j] as f64)-(keys[i][j] as f64))*(f as f64)+(keys[i][j] as f64)) as f32)
        } else {keys[keys.len()-1]};
        let r=if i<rotations.len()-1 {qlerp(rotations[i],rotations[i+1],t-i as f32)}
            else {rotations[rotations.len()-1]};
        (p,qmul(r,self.fix))
    }
    pub fn world(&self,d:f32,lane:f32)->V3 {
        let (o,[x,y,z,w])=self.transform(d);
        let l=lane*11.25;
        let [x2,y2,z2]=[x+x,y+y,z+z];let yy=y2*y;let zz=z2*z;
        let (l64,f)=(l as f64,|a:f32,b:f32,sgn:f64|a as f64+sgn*b as f64);
        [((1.0-f(yy,zz,1.0))*l64) as f32+o[0],(f(x2*y,z2*w,1.0)*l64) as f32+o[1],(f(x2*z,y2*w,-1.0)*l64) as f32+o[2]]
    }
}
