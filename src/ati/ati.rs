/* Defines all types used to perform dynamic ATI. Every type in this file
 * is also defined in the instrumented code by `types.rs`.
 *
 * Key points include: 
 * 1. `struct ATI` - A single global instance of this struct exists in the program
 *    accessible everywhere within the instrumented files, which holds the value_uf
 *    UnionFind (tracking all value interaction, globally) alongside the actual 
 *    abstract type partition at each site. All interactions with ATI instrumentation
 *    are done by calling methods associated with this struct.
 * 2. `struct TaggedValue<T>` - a tuple of (T, Id), which implements all necessary
 *    operators on T to record interactions within the value_uf, when they happen.
 * 3. `struct Site` - A program point, created in stubs, which stores the abstract 
 *    types of variables registered to it.
 * 4. `struct Sites` - Maintains a collection of program points, all the sites in the 
 *    instrumented file.
 * 5. `struct UnionFind` - A simple union find data structure, with some classic rank
 *    optimization.
*/

pub type Id = u64;

/// Top-level global that owns all information about all value interactions
/// and ATI site states.
pub static ATI_ANALYSIS: std::sync::LazyLock<std::sync::Arc<std::sync::Mutex<ATI>>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(std::sync::Mutex::new(ATI::new())));

/// Generates incrementing tags of type `Id`, with each call to `tag()`
#[derive(Debug)]
pub struct Tagger {
    next_id: Id,
}

impl Tagger {
    /// Creates a new Tagger
    pub fn new() -> Self {
        Tagger { next_id: 0 }
    }

    /// Fetches the next tag
    pub fn tag(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;

        id
    }
}

/// A tuple of a primative type T, alongside a unique `Id`.
/// This isn't expected to be created directly, but is instead
/// used as a return type from `ATI::track`.
///
/// Further, this struct implements `std::ops::{Add, Sub, Mul, Div}`,
/// alongside Ord, and Eq for less than and comparison,
/// as long as `T` implements each operator. Whenever two tagged values
/// are observed interacting through these operators, global `ATI_ANALYSIS`
/// is updated to record the interaction.
#[derive(Debug, Clone, Copy)]
pub struct TaggedValue<T: Copy>(pub T, pub Id);

impl<T> TaggedValue<T>
where
    T: Copy,
{
    /// Creates a new TaggedValue, given the parameters
    pub fn new(value: T, id: Id) -> Self {
        Self(value, id)
    }

    /// Copies the value out of the struct
    pub fn unbind(&self) -> T {
        self.0
    }
}

// Mostly for debugging purposes
impl<T> std::fmt::Display for TaggedValue<T>
where
    T: Copy + std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}

/// View TaggedValue docstring.
impl<T> std::ops::Add<TaggedValue<T>> for TaggedValue<T>
where
    T: std::ops::Add<Output = T> + Copy,
{
    type Output = TaggedValue<T>;

    fn add(self, rhs: TaggedValue<T>) -> Self::Output {
        let res = ATI::track(self.0 + rhs.0);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Sub<TaggedValue<T>> for TaggedValue<T>
where
    T: std::ops::Sub<Output = T> + Copy,
{
    type Output = TaggedValue<T>;

    fn sub(self, rhs: Self) -> Self::Output {
        let res = ATI::track(self.0 - rhs.0);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Mul<TaggedValue<T>> for TaggedValue<T>
where
    T: std::ops::Mul<Output = T> + Copy,
{
    type Output = TaggedValue<T>;

    fn mul(self, rhs: Self) -> Self::Output {
        let res = ATI::track(self.0 * rhs.0);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Div<TaggedValue<T>> for TaggedValue<T>
where
    T: std::ops::Div<Output = T> + Copy,
{
    type Output = TaggedValue<T>;

    fn div(self, rhs: Self) -> Self::Output {
        let res = ATI::track(self.0 / rhs.0);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> PartialEq for TaggedValue<T>
where
    T: Copy + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        ATI_ANALYSIS.lock().unwrap().union_tags(&self, &other);
        self.0 == other.0
    }
}
impl<T> Eq for TaggedValue<T> where T: Copy + PartialEq {}

impl<T> PartialOrd for TaggedValue<T>
where
    T: Copy + PartialEq + PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        ATI_ANALYSIS.lock().unwrap().union_tags(&self, other);
        match self.0.partial_cmp(&other.0) {
            Some(core::cmp::Ordering::Equal) => Some(core::cmp::Ordering::Equal),
            ord => return ord,
        }
    }
}

impl<T> Ord for TaggedValue<T>
where
    T: Copy + Ord,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        ATI_ANALYSIS.lock().unwrap().union_tags(&self, other);
        self.0.cmp(&other.0)
    }
}

impl<T> std::hash::Hash for TaggedValue<T> 
where 
    T: Copy + std::hash::Hash,
{
    fn hash<H>(&self, hasher: &mut H) where H: std::hash::Hasher { 
        self.0.hash(hasher)
    }
}

/// Represents a Site under analysis, ultimately a mapping of in-scope
/// variables to thier values at the start and end of each function.
#[derive(Debug)]
pub struct Site {
    type_uf: UnionFind,
    var_tags: std::collections::BTreeMap<String, Id>,
    observed_var_tags: Vec<(String, Id)>,
    name: String, // Debug information
}

impl Site {
    /// Creates a new empty Site.
    pub fn new(name: &str) -> Self {
        Site {
            type_uf: UnionFind::new(),
            var_tags: std::collections::BTreeMap::new(),
            observed_var_tags: Vec::new(),
            name: name.to_owned(),
        }
    }

    /// Records that a particular `tv: TaggedValue<T>` was bound to a variable
    /// named `var_name`.
    ///
    /// Intended for use whenever a let binding occurs. Essentially, abusing
    /// some notation, 1 gets converted to 2. Can also be used to record parameters
    /// or return values, as long as it's properly formatted.
    /// ```
    /// 1. let x = 10;
    /// 2. let x = site.bind("x", TaggedValue<10>)
    /// ```
    pub fn bind<T>(&mut self, var_name: &str, tv: TaggedValue<T>) -> TaggedValue<T>
    where
        T: Copy,
    {
        self.observed_var_tags.push((var_name.into(), tv.1));
        tv
    }

    /// Algorithm from paper, updates ATI information based on observed_vars
    pub fn update(&mut self, value_uf: &mut UnionFind) {
        for (new_var, new_var_tag) in &self.observed_var_tags {
            let new_leader_tag = value_uf.find(new_var_tag).unwrap(); // ? is this unwrap safe? 
            let new_leader_tag = self.type_uf.introduce_tag(new_leader_tag);

            if let Some(old_tag) = self.var_tags.get(new_var) {
                let old_leader_tag = value_uf.find(old_tag).unwrap();

                let merged = self
                    .type_uf
                    .union_tags(&old_leader_tag, &new_leader_tag)
                    .unwrap();
                self.var_tags.insert(new_var.clone(), merged);
            } else {
                self.var_tags.insert(new_var.clone(), new_leader_tag);
            }
        }

        self.observed_var_tags.clear(); // done merging these vars in!
    }

    /// Produces ATI output, called at the end of main.
    pub fn report(&self) {
        println!("{}", self.name);
        for (var, tag) in self.var_tags.iter() {
            println!("{var}:{tag:?}");
        }
        println!("---");
    }
}

/// Manages multiple Sites at once, to allow for analyzing multiple functions
pub struct Sites {
    locs: std::collections::BTreeMap<String, Site>,
}
impl Sites {
    pub fn new() -> Self {
        Sites {
            locs: std::collections::BTreeMap::new(),
        }
    }

    /// Pulls a site out of the map, for local modification.
    /// If no site with `id` exists, creates a new one.
    pub fn extract(&mut self, id: &str) -> Site {
        if !self.locs.contains_key(id) {
            Site::new(id)
        } else {
            self.locs.remove(id).unwrap()
        }
    }

    /// Puts a site that was locally modified back into the map.
    pub fn stash(&mut self, site: Site) {
        self.locs.insert(site.name.clone(), site);
    }

    /// Output results for all analyzed sites.
    pub fn report(&self) {
        println!("===ATI-ANALYSIS-START===");
        for (_, site) in self.locs.iter() {
            site.report();
        }
    }
}

/// Basic UnionFind implementation, with some light rank optimization.
#[derive(Debug)]
pub struct UnionFind {
    id_to_index: std::collections::HashMap<Id, usize>,
    pub index_to_set: Vec<Id>,
    parent: Vec<usize>,
    rank: Vec<usize>,
    tagger: Tagger,
}

impl UnionFind {
    /// Constructor
    pub fn new() -> Self {
        Self {
            id_to_index: std::collections::HashMap::new(),
            index_to_set: Vec::new(),
            parent: Vec::new(),
            rank: Vec::new(),
            tagger: Tagger::new(),
        }
    }

    /// Creates a new set in the union find, returning
    /// an Id that corresponds to it.
    pub fn make_set(&mut self) -> Id {
        let id = self.tagger.tag();
        self.introduce_tag(id)
    }

    /// Adds the passed in id to the UnionFind, in it's own set.
    /// If a set already exists for this Id, does nothing.
    pub fn introduce_tag(&mut self, id: Id) -> Id {
        if self.id_to_index.contains_key(&id) {
            return id;
        }

        let index = self.parent.len();
        self.id_to_index.insert(id.clone(), index);
        self.index_to_set.push(id.clone());
        self.parent.push(index);
        self.rank.push(0);

        return id;
    }

    /// Gets the index in parent associated with this id.
    fn get_index(&self, id: &Id) -> Option<usize> {
        self.id_to_index.get(id).copied()
    }

    /// Finds the parent Id which represents the leader of the set
    /// which contains `id`.
    pub fn find(&mut self, id: &Id) -> Option<Id> {
        let index = self.get_index(id)?;
        let leader_index = self.find_index(index);
        Some(self.index_to_set[leader_index].clone())
    }

    /// Associates the set represented by id1 and id2
    pub fn union_tags(&mut self, id1: &Id, id2: &Id) -> Option<Id> {
        let i1 = self.get_index(id1)?;
        let i2 = self.get_index(id2)?;
        let leader_index = self.union_indices(i1, i2);
        Some(self.index_to_set[leader_index].clone())
    }

    /// Finds the parent index of the set at index `x` of self.parent
    fn find_index(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find_index(self.parent[x]);
        }
        self.parent[x]
    }

    /// Associates the indecies `x` and `y` together, putting them
    /// in the same set.
    fn union_indices(&mut self, x: usize, y: usize) -> usize {
        let x_root = self.find_index(x);
        let y_root = self.find_index(y);

        if x_root == y_root {
            return x_root;
        }

        if self.rank[x_root] < self.rank[y_root] {
            self.parent[x_root] = y_root;
            y_root
        } else if self.rank[x_root] > self.rank[y_root] {
            self.parent[y_root] = x_root;
            x_root
        } else {
            self.parent[y_root] = x_root;
            self.rank[x_root] += 1;
            x_root
        }
    }
}

/// This struct owns all necessary information for analysis.
pub struct ATI {
    value_uf: UnionFind,
    sites: Sites,
}

impl ATI {
    /// Intializes a new global ATI tracker.
    pub fn new() -> Self {
        Self {
            value_uf: UnionFind::new(),
            sites: Sites::new(),
        }
    }

    /// Moves a value from a standard type T to a TaggedValue<T>,
    /// assigning it a unique Id
    pub fn track<T>(value: T) -> TaggedValue<T>
    where
        T: Copy,
    {
        let id = ATI_ANALYSIS.lock().unwrap().value_uf.make_set();
        TaggedValue::new(value, id)
    }

    /// Fetches a site, or creates it, with the given name.
    pub fn get_site(&mut self, name: &str) -> Site {
        self.sites.extract(name)
    }

    /// Update abstract types at this site, then store it back
    /// into the map. Call whenever you are done registering variables to a site.
    pub fn update_site(&mut self, mut site: Site) {
        site.update(&mut self.value_uf);
        self.sites.stash(site);
    }

    /// Observe two tagged values interacting together, merging them in 
    /// value_uf.
    pub fn union_tags<T>(&mut self, tv1: &TaggedValue<T>, tv2: &TaggedValue<T>)
    where
        T: Copy,
    {
        self.value_uf.union_tags(&tv1.1, &tv2.1);
    }

    /// Produce output partition that defines abstract types.
    pub fn report(&self) {
        self.sites.report();
    }
}
