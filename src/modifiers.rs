#[derive(Clone,Copy,Debug)]
pub enum Operation {Add,Set}
#[derive(Clone,Debug)]
pub struct Modifier {pub index:usize,pub value:f32,pub operation:Operation,pub owner:u64,pub once:bool,pub remaining:Option<f32>,
    pub lb:Option<[f32;5]>}
#[derive(Clone,Default,Debug)]
pub struct Modifiers {pub entries:Vec<Modifier>}
impl Modifiers {
    pub fn add(&mut self,index:usize,value:f32,operation:Operation,owner:u64,once:bool) {
        self.entries.push(Modifier{index,value,operation,owner,once,remaining:None,lb:None});
    }
    pub fn add_timed(&mut self,index:usize,value:f32,operation:Operation,owner:u64,once:bool,remaining:f32) {
        self.entries.push(Modifier{index,value,operation,owner,once,remaining:Some(remaining),lb:None});
    }
    pub fn update_timed(&mut self,dt:f32) {
        self.entries.retain_mut(|m|match &mut m.remaining {
            Some(left) if *left>=0.0=>{*left-=dt;*left>0.0},
            _=>true,
        });
    }
    pub fn apply(&self,index:usize,mut value:f32)->f32 {
        for m in self.entries.iter().filter(|m|m.index==index && !m.once) {
            value=match m.operation {Operation::Add=>value+m.value,Operation::Set=>m.value};
        } value
    }
    pub fn apply_sum(&self,index:usize,value:f32)->f32 {
        if self.entries.iter().any(|m|m.index==index && !m.once && matches!(m.operation,Operation::Set)) {return self.apply(index,value);}
        let mut sum=0.0f32;let mut any=false;
        for m in self.entries.iter().filter(|m|m.index==index && !m.once) {sum+=m.value;any=true;}
        if any {value+sum} else {value}
    }
    pub fn consume(&mut self,index:usize,mut value:f32)->f32 {
        for m in self.entries.iter().filter(|m|m.index==index && m.once) {
            value=match m.operation {Operation::Add=>value+m.value,Operation::Set=>m.value};
        }
        self.entries.retain(|m|!(m.index==index && m.once));value
    }
    pub fn stop(&mut self,owner:u64) {self.entries.retain(|m|m.owner!=owner || m.remaining.is_some());}
    pub fn has(&self,index:usize)->bool {self.entries.iter().any(|m|m.index==index)}
    pub fn set_last_lb(&mut self,lb:[f32;5]) {if let Some(m)=self.entries.last_mut() {m.lb=Some(lb);}}
}
