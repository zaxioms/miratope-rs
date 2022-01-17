//! The code used to get the symmetry of a polytope and do operations based on that.

use std::{collections::{BTreeMap, HashMap, HashSet}, vec, iter::FromIterator};

use crate::{
    abs::{Abstract, Element, ElementList, Ranked, Ranks, Subelements, Superelements, AbstractBuilder},
    conc::Concrete,
    float::Float,
    group::{Group, GenIter}, geometry::{Matrix, PointOrd, Subspace}, Polytope,
};

use vec_like::*;

/// Input for the faceting function
pub enum GroupEnum {
    /// Group of matrices
    ConcGroup(Group<GenIter<Matrix<f64>>>),
    /// Group of vertex mappings
    VertexMap(Vec<Vec<usize>>),
    /// True: take chiral group
    /// False: take full group
    Chiral(bool),
}

fn faceting_subdim(rank: usize, plane: Subspace<f64>, points: Vec<PointOrd<f64>>, vertex_map: Vec<Vec<usize>>, edge_length: Option<f64>, irc: bool) ->
    (Vec<(Ranks, Vec<(usize, usize)>)>, // Vec of facetings, along with the facet types of each of them
    Vec<usize>, // Counts of each hyperplane orbit
    Vec<Vec<Ranks>> // Possible facets, these will be the possible ridges one dimension up
) {
    let total_vert_count = points.len();

    if rank == 2 {
        // The only faceting of a dyad is itself.
        // We distinguish between snub and non-snub edges.

        let mut snub = true;

        for row in &vertex_map {
            if row[0] == 1 {
                snub = false;
                break
            }
        }

        if snub {
            return (
                vec![(Abstract::dyad().ranks().clone(), vec![(0,0), (1,0)])],
                vec![1,1],
                vec![
                    vec![vec![
                        vec![].into(),
                        vec![
                            Element::new(vec![0].into(), vec![].into())
                            ].into(),
                        vec![
                            Element::new(vec![0].into(), vec![].into())
                            ].into(),
                    ].into()],
                    vec![vec![
                        vec![].into(),
                        vec![
                            Element::new(vec![0].into(), vec![].into())
                            ].into(),
                        vec![
                            Element::new(vec![1].into(), vec![].into())
                            ].into(),
                    ].into()]
                    ]
            )
        }
        else {
            return (
                vec![(Abstract::dyad().ranks().clone(), vec![(0,0)])],
                vec![2],
                vec![
                    vec![vec![
                        vec![].into(),
                        vec![
                            Element::new(vec![0].into(), vec![].into())
                            ].into(),
                        vec![
                            Element::new(vec![0].into(), vec![].into())
                            ].into(),
                    ].into()]
                    ]
            )
        }
    }

    let mut flat_points = Vec::new();
    for p in &points {
        flat_points.push(PointOrd::new(plane.flatten(&p.0)));
    }
    
    let mut vertex_orbits = Vec::new(); // Vec of orbits which are vecs of vertices.
    let mut orbit_of_vertex = vec![0; total_vert_count]; // For each vertex stores its orbit index.
    let mut checked_vertices = vec![false; total_vert_count]; // Stores whether we've already checked the vertex.

    let mut orbit_idx = 0;
    for v in 0..total_vert_count {
        if !checked_vertices[v] {
            // We found a new orbit of vertices.
            let mut new_orbit = Vec::new();
            for row in &vertex_map {
                // Find all vertices in the same orbit.
                let c = row[v];
                if !checked_vertices[c] {
                    new_orbit.push(c);
                    checked_vertices[c] = true;
                    orbit_of_vertex[c] = orbit_idx;
                }
            }
            vertex_orbits.push(new_orbit);
            orbit_idx += 1;
        }
    }

    let mut pair_orbits = Vec::new();
    let mut checked = vec![vec![false; total_vert_count]; total_vert_count];
    
    for orbit in vertex_orbits {
        let rep = orbit[0]; // We only need one representative per orbit.
        for vertex in 0..total_vert_count {
            if vertex != rep && !checked[rep][vertex] {
                if let Some(e_l) = edge_length {
                    if ((&points[vertex].0-&points[rep].0).norm() - e_l).abs() > f64::EPS {
                        continue
                    }
                }
                let mut new_orbit = Vec::new();
                for row in &vertex_map {
                    let (c1, c2) = (row[rep], row[vertex]);
                    if !checked[c1][c2] {
                        new_orbit.push(vec![c1, c2]);
                        checked[c1][c2] = true;
                        checked[c2][c1] = true;
                    }
                }
                pair_orbits.push(new_orbit);
            }
        }
    }

    // Enumerate hyperplanes
    let mut hyperplane_orbits = Vec::new();
    let mut checked = HashSet::<Vec<usize>>::new();
    let mut hyperplanes_vertices = Vec::new();

    for pair_orbit in pair_orbits {
        let rep = &pair_orbit[0];

        let mut new_vertices = vec![0; rank-3];
        let mut update = 0;
        if rank > 3 {
            update = rank-4;
        }
        'b: loop {
            'c: loop {
                if let Some(e_l) = edge_length {
                    for v in &new_vertices {
                        if ((&points[*v].0-&points[rep[0]].0).norm() - e_l).abs() > f64::EPS {
                            break 'c;
                        }
                    }
                }
                // We start with a pair and add enough vertices to define a hyperplane.
                let mut tuple = rep.clone();
                tuple.append(&mut new_vertices.clone());

                let mut first_points = Vec::new();
                for v in tuple {
                    first_points.push(&flat_points[v].0);
                }

                let hyperplane = Subspace::from_points(first_points.clone().into_iter());
                if hyperplane.is_hyperplane() {

                    let mut hyperplane_vertices = Vec::new();
                    for (idx, v) in flat_points.iter().enumerate() {
                        if hyperplane.distance(&v.0) < f64::EPS {
                            hyperplane_vertices.push(idx);
                        }
                    }
                    hyperplane_vertices.sort_unstable();

                    // Check if the hyperplane has been found already.
                    if !checked.contains(&hyperplane_vertices) {
                        // If it's new, we add all the ones in its orbit.
                            let mut new_orbit = Vec::new();
                            let mut new_orbit_vertices = Vec::new();
                            for row in &vertex_map {
                                let mut new_hp_v = Vec::new();
                                for idx in &hyperplane_vertices {
                                    new_hp_v.push(row[*idx]);
                                }
                                let new_hp_points = new_hp_v.iter().map(|x| &flat_points[*x].0);
                                let new_hp = Subspace::from_points(new_hp_points);

                                let mut sorted = new_hp_v.clone();
                                sorted.sort_unstable();

                                if !checked.contains(&sorted) {
                                    checked.insert(sorted);
                                    new_orbit.push(new_hp);
                                    new_orbit_vertices.push(new_hp_v);
                                }
                            }
                            
                            hyperplane_orbits.push(new_orbit);
                            hyperplanes_vertices.push(new_orbit_vertices);
                    }
                    if rank <= 3 {
                        break 'b;
                    }
                }
                break
            }
            loop { // Increment new_vertices.
                if new_vertices[update] == total_vert_count + update - rank + 3 {
                    if update < 1 {
                        break 'b;
                    }
                    else {
                        update -= 1;
                    }
                } else {
                    new_vertices[update] += 1;
                    for i in update+1..rank-3 {
                        new_vertices[i] = new_vertices[i-1]+1;
                    }
                    update = rank-4;
                    break;
                }
            }
        }
    }
    // Facet the hyperplanes
    let mut possible_facets = Vec::new();
    let mut possible_facets_global: Vec<Vec<(Ranks, Vec<(usize,usize)>)>> = Vec::new(); // copy of above but with semi-global vertex indices
    let mut ridges: Vec<Vec<Vec<Ranks>>> = Vec::new();
    let mut ff_counts = Vec::new();

    for (i, orbit) in hyperplane_orbits.iter().enumerate() {
        let (hp, hp_v) = (orbit[0].clone(), hyperplanes_vertices[i][0].clone());
        let mut stabilizer = Vec::new();
        for row in &vertex_map {
            let mut slice = Vec::new();
            for v in &hp_v {
                slice.push(row[*v]);
            }
            let mut slice_sorted = slice.clone();
            slice_sorted.sort_unstable();

            if slice_sorted == hp_v {
                stabilizer.push(slice.clone());
            }
        }

        // Converts global vertex indices to local ones.
        let mut map_back = BTreeMap::new();
        for (idx, el) in stabilizer[0].iter().enumerate() {
            map_back.insert(*el, idx);
        }
        
        let mut new_stabilizer = stabilizer.clone();

        for a in 0..stabilizer.len() {
            for b in 0..stabilizer[a].len() {
                new_stabilizer[a][b] = *map_back.get(&stabilizer[a][b]).unwrap();
            }
        }

        let mut points = Vec::new();
        for v in &hp_v {
            points.push(flat_points[*v].clone());
        }

        let (possible_facets_row, ff_counts_row, ridges_row) =
            faceting_subdim(rank-1, hp, points, new_stabilizer.clone(), edge_length, irc);

        let mut possible_facets_global_row = Vec::new();
        for f in &possible_facets_row {
            let mut new_f = f.clone();
            let mut new_edges = ElementList::new();
            for v in f.0[2].clone() {
                // Converts indices back to semi-global
                let mut new_edge = Element::new(vec![].into(), vec![].into());
                for s in v.subs {
                    new_edge.subs.push(hp_v[s]);
                }
                new_edges.push(new_edge);
            }
            new_f.0[2] = new_edges;

            possible_facets_global_row.push(new_f);
        }
        possible_facets.push(possible_facets_row);
        possible_facets_global.push(possible_facets_global_row);
        ridges.push(ridges_row);
        ff_counts.push(ff_counts_row);
    }

    let mut ridge_idx_orbits = Vec::new();
    let mut ridge_orbits = HashMap::new();
    let mut ridge_counts = Vec::new(); // Counts the number of ridges in each orbit
    let mut orbit_idx = 0;

    let mut hp_i = 0; // idk why i have to do this, thanks rust
    for ridges_row in ridges {
        let mut r_i_o_row = Vec::new();

        for ridges_row_row in ridges_row {
            let mut r_i_o_row_row = Vec::new();

            for mut ridge in ridges_row_row {
                // goes through all the ridges

                // globalize
                let mut new_list = ElementList::new();
                for i in 0..ridge[2].len() {
                    let mut new = Element::new(Subelements::new(), Superelements::new());
                    for sub in &ridge[2][i].subs {
                        new.subs.push(hyperplanes_vertices[hp_i][0][*sub])
                    }
                    new_list.push(new);
                }
                ridge[2] = new_list;

                ridge.element_sort_strong();

                match ridge_orbits.get(&ridge) {
                    Some(idx) => {
                        // writes the orbit index at the ridge index
                        r_i_o_row_row.push(*idx);
                    }
                    None => {
                        // adds all ridges with the same orbit to the map
                        let mut count = 0;
                        for row in &vertex_map {
                            let mut new_ridge = ridge.clone();

                            let mut new_list = ElementList::new();
                            for i in 0..new_ridge[2].len() {
                                let mut new = Element::new(Subelements::new(), Superelements::new());
                                for sub in &ridge[2][i].subs {
                                    new.subs.push(row[*sub])
                                }
                                new_list.push(new);
                            }
                            new_ridge[2] = new_list;

                            new_ridge.element_sort_strong();

                            if ridge_orbits.get(&new_ridge).is_none() {
                                ridge_orbits.insert(new_ridge, orbit_idx);
                                count += 1;
                            }
                        }
                        r_i_o_row_row.push(orbit_idx);
                        ridge_counts.push(count);
                        orbit_idx += 1;
                    }
                }
            }
            r_i_o_row.push(r_i_o_row_row);
        }
        ridge_idx_orbits.push(r_i_o_row);
        hp_i += 1;
    }

    let mut f_counts = Vec::new();
    for orbit in hyperplane_orbits {
        f_counts.push(orbit.len());
    }

    // Actually do the faceting
    let mut output = Vec::new();

    let mut facets = vec![(0, 0)];

    'l: loop {
        loop {
            let t = facets.last_mut().unwrap();
            if t.0 >= possible_facets.len() {
                facets.pop();
                if facets.is_empty() {
                    break 'l;
                }
                let t2 = facets.last_mut().unwrap();
                if t2.1 + 1 >= possible_facets[t2.0].len() {
                    t2.0 += 1;
                    t2.1 = 0;
                }
                else {
                    t2.1 += 1;
                }
            }
            else if t.1 >= possible_facets[t.0].len() {
                t.0 += 1;
                t.1 = 0;
            }
            else {
                break
            }
        }
        let mut ridges = vec![0; ridge_counts.len()];    

        'a: for facet in &facets {
            let hp = facet.0;
            let f = facet.1;
            let f_count = f_counts[hp];

            let ridge_idxs_local = &possible_facets[hp][f].1;
            for ridge_idx in ridge_idxs_local {
                let ridge_orbit = ridge_idx_orbits[hp][ridge_idx.0][ridge_idx.1];
                let ridge_count = ff_counts[hp][ridge_idx.0];
                let total_ridge_count = ridge_counts[ridge_orbit];
                let mul = f_count * ridge_count / total_ridge_count;

                ridges[ridge_orbit] += mul;
                if ridges[ridge_orbit] > 2 {
                    break 'a;
                }
            }
        }
        let mut valid = 0; // 0: valid, 1: exotic, 2: incomplete
        for r in ridges {
            if r > 2 {
                valid = 1;
                break
            }
            if r == 1 {
                valid = 2;
            }
        }
        match valid {
            0 => {
                // Output the faceted polytope. We will build it from the set of its facets.

                let mut facet_set = HashSet::new();
                for facet_orbit in &facets {
                    let facet = &possible_facets_global[facet_orbit.0][facet_orbit.1].0;
                    let facet_local = &possible_facets[facet_orbit.0][facet_orbit.1].0;
                    for row in &vertex_map {
                        let mut new_facet = facet.clone();
                            
                        let mut new_list = ElementList::new();
                        for i in 0..facet[2].len() {
                            let mut new = Element::new(Subelements::new(), Superelements::new());
                            for sub in &facet[2][i].subs {
                                new.subs.push(row[*sub])
                            }
                            new_list.push(new);
                        }
                        new_facet[2] = new_list;

                        new_facet.element_sort_strong_with_local(facet_local);
                        facet_set.insert(new_facet);
                    }
                }

                let mut facet_vec = Vec::from_iter(facet_set);

                let mut ranks = Ranks::new();
                ranks.push(vec![Element::new(vec![].into(), vec![].into())].into()); // nullitope
                ranks.push(vec![Element::new(vec![0].into(), vec![].into()); total_vert_count].into()); // vertices

                for r in 2..rank-1 { // edges and up
                    let mut subs_to_idx = HashMap::new();
                    let mut idx_to_subs = Vec::new();
                    let mut idx = 0;

                    for facet in &facet_vec {
                        let els = &facet[r];
                        for el in els {
                            if subs_to_idx.get(&el.subs).is_none() {
                                subs_to_idx.insert(el.subs.clone(), idx);
                                idx_to_subs.push(el.subs.clone());
                                idx += 1;
                            }
                        }
                    }
                    for i in 0..facet_vec.len() {
                        let mut new_list = ElementList::new();
                        for j in 0..facet_vec[i][r+1].len() {
                            let mut new = Element::new(Subelements::new(), Superelements::new());
                            for sub in &facet_vec[i][r+1][j].subs {
                                let sub_subs = &facet_vec[i][r][*sub].subs;
                                new.subs.push(*subs_to_idx.get(sub_subs).unwrap())
                            }
                            new_list.push(new);
                        }
                        facet_vec[i][r+1] = new_list;
                    }

                    let mut new_rank = ElementList::new();
                    for el in idx_to_subs {
                        new_rank.push(Element::new(el, vec![].into()));
                    }
                    ranks.push(new_rank);
                }
                let mut new_rank = ElementList::new();
                let mut set = HashSet::new();

                for f_i in 0..facet_vec.len() {
                    facet_vec[f_i][rank-1][0].subs.sort();
                    let subs = facet_vec[f_i][rank-1][0].subs.clone();
                    if !set.contains(&subs) {
                        new_rank.push(Element::new(subs.clone(), Superelements::new()));
                        set.insert(subs);
                    }
                }
                let n_r_len = new_rank.len();
                ranks.push(new_rank); // facets

                ranks.push(vec![Element::new(Subelements::from_iter(0..n_r_len), Superelements::new())].into()); // body

                output.push((ranks, facets.clone()));

                if irc {
                    let t = facets.last().unwrap().clone();
                    facets.push((t.0 + 1, 0));
                } else {
                    let t = facets.last_mut().unwrap();
                    if t.1 == possible_facets[t.0].len() - 1 {
                        t.0 += 1;
                        t.1 = 0;
                    }
                    else {
                        t.1 += 1;
                    }
                }
            }
            1 => {
                let t = facets.last_mut().unwrap();
                if t.1 == possible_facets[t.0].len() - 1 {
                    t.0 += 1;
                    t.1 = 0;
                }
                else {
                    t.1 += 1;
                }
            }
            2 => {
                let t = facets.last().unwrap().clone();
                facets.push((t.0 + 1, 0));
            }
            _ => {}
        }
    }
    let mut output_ridges = Vec::new();
    for i in possible_facets_global {
        let mut a = Vec::new();
        for j in i {
            a.push(j.0);
        }
        output_ridges.push(a);
    }

    return (output, f_counts, output_ridges)
}

impl Concrete {
    /// Enumerates the facetings of a polytope under a provided symmetry group or vertex map.
    /// If the symmetry group is not provided, it uses the full symmetry of the polytope.
    pub fn faceting(&mut self, symmetry: GroupEnum, edge_length: Option<f64>, noble: Option<usize>, irc: bool) -> Vec<Concrete> {
        let rank = self.rank();

        let mut vertices_ord = Vec::<PointOrd<f64>>::new();
        for v in &self.vertices {
            vertices_ord.push(PointOrd::new(v.clone()));
        }
        let vertices = BTreeMap::from_iter((vertices_ord.clone()).into_iter().zip(0..));

        let vertex_map = match symmetry {
            GroupEnum::ConcGroup(group) => {
                println!("Computing vertex map...");
                self.get_vertex_map(group)
            },
            GroupEnum::VertexMap(a) => a,
            GroupEnum::Chiral(chiral) => {
                if chiral {
                    println!("Computing rotation symmetry group...");
                    let g = self.get_rotation_group();
                    println!("Rotation symmetry order {}", g.0.count());
                    g.1
                }
                else {
                    println!("Computing symmetry group...");
                    let g = self.get_symmetry_group();
                    println!("Symmetry order {}", g.0.count());
                    g.1
                }
            },
        };

        println!("Enumerating hyperplanes...");
        
        // Checking every r-tuple of vertices would take too long, so we put pairs into orbits first to reduce the number.
        // I don't think we need to store the whole orbits at this point, but they might be useful if we want to improve the algorithm.
        let mut vertex_orbits = Vec::new(); // Vec of orbits which are vecs of vertices.
        let mut orbit_of_vertex = vec![0; vertices.len()]; // For each vertex stores its orbit index.
        let mut checked_vertices = vec![false; vertices.len()]; // Stores whether we've already checked the vertex.

        let mut orbit_idx = 0;
        for v in 0..vertices.len() {
            if !checked_vertices[v] {
                // We found a new orbit of vertices.
                let mut new_orbit = Vec::new();
                for row in &vertex_map {
                    // Find all vertices in the same orbit.
                    let c = row[v];
                    if !checked_vertices[c] {
                        new_orbit.push(c);
                        checked_vertices[c] = true;
                        orbit_of_vertex[c] = orbit_idx;
                    }
                }
                vertex_orbits.push(new_orbit);
                orbit_idx += 1;
            }
        }

        let mut pair_orbits = Vec::new();
        let mut checked = vec![vec![false; vertices.len()]; vertices.len()];
        
        for orbit in vertex_orbits {
            let rep = orbit[0]; // We only need one representative per orbit.
            for vertex in 0..vertices.len() {
                if vertex != rep && !checked[rep][vertex] {
                    if let Some(e_l) = edge_length {
                        if ((&self.vertices[vertex]-&self.vertices[rep]).norm() - e_l).abs() > f64::EPS {
                            continue
                        }
                    }
                    let mut new_orbit = Vec::new();
                    for row in &vertex_map {
                        let (c1, c2) = (row[rep], row[vertex]);
                        if !checked[c1][c2] {
                            new_orbit.push(vec![c1, c2]);
                            checked[c1][c2] = true;
                            checked[c2][c1] = true;
                        }
                    }
                    pair_orbits.push(new_orbit);
                }
            }
        }

        // Enumerate hyperplanes
        let mut hyperplane_orbits = Vec::new();
        let mut checked = HashSet::new();
        let mut hyperplanes_vertices = Vec::new();

        for pair_orbit in pair_orbits {
            let rep = &pair_orbit[0];

            let mut new_vertices = vec![0; rank-3];
            let mut update = rank-4;
            'b: loop {
                'c: loop {
                    if let Some(e_l) = edge_length {
                        for v in &new_vertices {
                            if ((&self.vertices[*v]-&self.vertices[rep[0]]).norm() - e_l).abs() > f64::EPS {
                                break 'c;
                            }
                        }
                    }
                    // We start with a pair and add enough vertices to define a hyperplane.
                    let mut tuple = rep.clone();
                    tuple.append(&mut new_vertices.clone());

                    let mut points = Vec::new();
                    for v in tuple {
                        points.push(self.vertices[v].clone());
                    }

                    let hyperplane = Subspace::from_points(points.iter());
                    if hyperplane.is_hyperplane() {
                        let mut hyperplane_vertices = Vec::new();
                        for (idx, v) in self.vertices.iter().enumerate() {
                            if hyperplane.distance(&v) < f64::EPS {
                                hyperplane_vertices.push(idx);
                            }
                        }
                        hyperplane_vertices.sort_unstable();

                        // Check if the hyperplane has been found already.
                        if !checked.contains(&hyperplane_vertices) {
                            // If it's new, we add all the ones in its orbit.
                            let mut new_orbit = Vec::new();
                            let mut new_orbit_vertices = Vec::new();
                            for row in &vertex_map {
                                let mut new_hp_v = Vec::new();
                                for idx in &hyperplane_vertices {
                                    new_hp_v.push(row[*idx]);
                                }
                                let mut sorted = new_hp_v.clone();
                                sorted.sort_unstable();

                                if !checked.contains(&sorted) {
                                    let new_hp_points = new_hp_v.iter().map(|x| &self.vertices[*x]);
                                    let new_hp = Subspace::from_points(new_hp_points);
                                    checked.insert(sorted);
                                    new_orbit.push(new_hp);
                                    new_orbit_vertices.push(new_hp_v);
                                }
                            }
                            
                            hyperplane_orbits.push(new_orbit);
                            hyperplanes_vertices.push(new_orbit_vertices);
                        }
                    }
                    break
                }
                loop { // Increment new_vertices.
                    if new_vertices[update] == self.vertices.len() + update - rank + 3 {
                        if update < 1 {
                            break 'b;
                        }
                        else {
                            update -= 1;
                        }
                    } else {
                        new_vertices[update] += 1;
                        for i in update+1..rank-3 {
                            new_vertices[i] = new_vertices[i-1]+1;
                        }
                        update = rank-4;
                        break;
                    }
                }
            }
        }

        println!("Found {} hyperplanes in {} orbits", checked.len(), hyperplane_orbits.len());
        println!("Faceting hyperplanes...");

        // Facet the hyperplanes
        let mut possible_facets = Vec::new();
        let mut possible_facets_global: Vec<Vec<(Ranks, Vec<(usize,usize)>)>> = Vec::new(); // copy of above but with global vertex indices
        let mut ridges: Vec<Vec<Vec<Ranks>>> = Vec::new();
        let mut ff_counts = Vec::new();

        for (idx, orbit) in hyperplane_orbits.iter().enumerate() {
            let (hp, hp_v) = (orbit[0].clone(), hyperplanes_vertices[idx][0].clone());
            let mut stabilizer = Vec::new();
            for row in &vertex_map {
                let mut slice = Vec::new();
                for v in &hp_v {
                    slice.push(row[*v]);
                }
                let mut slice_sorted = slice.clone();
                slice_sorted.sort_unstable();

                if slice_sorted == hp_v {
                    stabilizer.push(slice.clone());
                }
            }

            // Converts global vertex indices to local ones.
            let mut map_back = BTreeMap::new();
            for (idx, el) in stabilizer[0].iter().enumerate() {
                map_back.insert(*el, idx);
            }
            let mut new_stabilizer = stabilizer.clone();
    
            for a in 0..stabilizer.len() {
                for b in 0..stabilizer[a].len() {
                    new_stabilizer[a][b] = *map_back.get(&stabilizer[a][b]).unwrap();
                }
            }
            
            let mut points = Vec::new();
            for v in &hp_v {
                points.push(vertices_ord[*v].clone());
            }

            let (possible_facets_row, ff_counts_row, ridges_row) =
                faceting_subdim(rank-1, hp, points, new_stabilizer, edge_length, irc);
            let mut possible_facets_global_row = Vec::new();
            for f in &possible_facets_row {
                let mut new_f = f.clone();
                let mut new_edges = ElementList::new();
                for v in f.0[2].clone() {
                    // Converts indices back to global
                    let mut new_edge = Element::new(vec![].into(), vec![].into());
                    for s in v.subs {
                        new_edge.subs.push(hp_v[s]);
                    }
                    new_edges.push(new_edge);
                }
                new_f.0[2] = new_edges;

                possible_facets_global_row.push(new_f);
            }
            possible_facets.push(possible_facets_row);
            possible_facets_global.push(possible_facets_global_row);
            ridges.push(ridges_row);
            ff_counts.push(ff_counts_row);

            println!("{}/{}", idx+1, hyperplane_orbits.len());
        }

        let mut ridge_idx_orbits = Vec::new();
        let mut ridge_orbits = HashMap::new();
        let mut ridge_counts = Vec::new(); // Counts the number of ridges in each orbit
        let mut orbit_idx = 0;

        let mut hp_i = 0; // idk why i have to do this, thanks rust
        for ridges_row in ridges {
            let mut r_i_o_row = Vec::new();

            for ridges_row_row in ridges_row {
                let mut r_i_o_row_row = Vec::new();

                for mut ridge in ridges_row_row {
                    // goes through all the ridges

                    // globalize
                    let mut new_list = ElementList::new();
                    for i in 0..ridge[2].len() {
                        let mut new = Element::new(Subelements::new(), Superelements::new());
                        for sub in &ridge[2][i].subs {
                            new.subs.push(hyperplanes_vertices[hp_i][0][*sub])
                        }
                        new_list.push(new);
                    }
                    ridge[2] = new_list;

                    ridge.element_sort_strong();

                    match ridge_orbits.get(&ridge) {
                        Some(idx) => {
                            // writes the orbit index at the ridge index
                            r_i_o_row_row.push(*idx);
                        }
                        None => {
                            // adds all ridges with the same orbit to the map
                            let mut count = 0;
                            for row in &vertex_map {
                                let mut new_ridge = ridge.clone();
                            
                                let mut new_list = ElementList::new();
                                for i in 0..new_ridge[2].len() {
                                    let mut new = Element::new(Subelements::new(), Superelements::new());
                                    for sub in &ridge[2][i].subs {
                                        new.subs.push(row[*sub])
                                    }
                                    new_list.push(new);
                                }
                                new_ridge[2] = new_list;

                                new_ridge.element_sort_strong();
                                if ridge_orbits.get(&new_ridge).is_none() {
                                    ridge_orbits.insert(new_ridge, orbit_idx);
                                    count += 1;
                                }
                            }
                            r_i_o_row_row.push(orbit_idx);
                            ridge_counts.push(count);
                            orbit_idx += 1;
                        }
                    }
                }
                r_i_o_row.push(r_i_o_row_row);
            }
            ridge_idx_orbits.push(r_i_o_row);
            hp_i += 1;
        }

        let mut f_counts = Vec::new();
        for orbit in hyperplane_orbits {
            f_counts.push(orbit.len());
        }

        // Actually do the faceting
        println!("Combining...");
        let mut output = Vec::new();

        let mut facets = vec![(0, 0)];

        'l: loop {
            loop {
                let t = facets.last_mut().unwrap();
                if t.0 >= possible_facets.len() {
                    facets.pop();
                    if facets.is_empty() {
                        break 'l;
                    }
                    let t2 = facets.last_mut().unwrap();
                    if t2.1 + 1 >= possible_facets[t2.0].len() {
                        t2.0 += 1;
                        t2.1 = 0;
                    }
                    else {
                        t2.1 += 1;
                    }
                }
                else if t.1 >= possible_facets[t.0].len() {
                    t.0 += 1;
                    t.1 = 0;
                }
                else {
                    break
                }
            }
            let mut ridges = vec![0; ridge_counts.len()];    

            'a: for facet in &facets {
                let hp = facet.0;
                let f = facet.1;
                let f_count = f_counts[hp];

                let ridge_idxs_local = &possible_facets[hp][f].1;
                for ridge_idx in ridge_idxs_local {
                    let ridge_orbit = ridge_idx_orbits[hp][ridge_idx.0][ridge_idx.1];
                    let ridge_count = ff_counts[hp][ridge_idx.0];
                    let total_ridge_count = ridge_counts[ridge_orbit];
                    let mul = f_count * ridge_count / total_ridge_count;

                    ridges[ridge_orbit] += mul;
                    if ridges[ridge_orbit] > 2 {
                        break 'a;
                    }
                }
            }
            let mut valid = 0; // 0: valid, 1: exotic, 2: incomplete
            for r in ridges {
                if r > 2 {
                    valid = 1;
                    break
                }
                if r == 1 {
                    valid = 2;
                }
            }
            match valid {
                0 => {
                    // Output the faceted polytope. We will build it from the set of its facets.

                    let mut facet_set = HashSet::new();
                    for facet_orbit in &facets {
                        let facet = &possible_facets_global[facet_orbit.0][facet_orbit.1].0;
                        let facet_local = &possible_facets[facet_orbit.0][facet_orbit.1].0;
                        for row in &vertex_map {
                            let mut new_facet = facet.clone();

                            let mut new_list = ElementList::new();
                            for i in 0..new_facet[2].len() {
                                let mut new = Element::new(Subelements::new(), Superelements::new());
                                for sub in &new_facet[2][i].subs {
                                    new.subs.push(row[*sub])
                                }
                                new_list.push(new);
                            }
                            new_facet[2] = new_list;

                            new_facet.element_sort_strong_with_local(facet_local);
                            facet_set.insert(new_facet);
                        }
                    }

                    let mut facet_vec = Vec::from_iter(facet_set);

                    let mut ranks = Ranks::new();
                    ranks.push(vec![Element::new(vec![].into(), vec![].into())].into()); // nullitope
                    ranks.push(vec![Element::new(vec![0].into(), vec![].into()); self.vertices.len()].into()); // vertices

                    for r in 2..rank-1 { // edges and up
                        let mut subs_to_idx = HashMap::new();
                        let mut idx_to_subs = Vec::new();
                        let mut idx = 0;

                        for facet in &facet_vec {
                            let els = &facet[r];
                            for el in els {
                                if subs_to_idx.get(&el.subs).is_none() {
                                    subs_to_idx.insert(el.subs.clone(), idx);
                                    idx_to_subs.push(el.subs.clone());
                                    idx += 1;
                                }
                            }
                        }
                        for i in 0..facet_vec.len() {
                            let mut new_list = ElementList::new();
                            for j in 0..facet_vec[i][r+1].len() {
                                let mut new = Element::new(Subelements::new(), Superelements::new());
                                for sub in &facet_vec[i][r+1][j].subs {
                                    let sub_subs = &facet_vec[i][r][*sub].subs;
                                    new.subs.push(*subs_to_idx.get(sub_subs).unwrap())
                                }
                                new_list.push(new);
                            }
                            facet_vec[i][r+1] = new_list;
                        }
                        let mut new_rank = ElementList::new();
                        for el in idx_to_subs {
                            new_rank.push(Element::new(el, vec![].into()));
                        }
                        ranks.push(new_rank);
                    }

                    let mut new_rank = ElementList::new();
                    let mut set = HashSet::new();

                    for f_i in 0..facet_vec.len() {
                        facet_vec[f_i][rank-1][0].subs.sort();
                        let subs = facet_vec[f_i][rank-1][0].subs.clone();
                        if !set.contains(&subs) {
                            new_rank.push(Element::new(subs.clone(), Superelements::new()));
                            set.insert(subs);
                        }
                    }
                    let n_r_len = new_rank.len();
                    ranks.push(new_rank); // facets
    
                    ranks.push(vec![Element::new(Subelements::from_iter(0..n_r_len), Superelements::new())].into()); // body
    
                    unsafe {
                        let mut builder = AbstractBuilder::new();
                        for rank in ranks {
                            builder.push_empty();
                            for el in rank {
                                builder.push_subs(el.subs);
                            }
                        }

                        if builder.ranks().is_dyadic().is_ok() {
                            let abs = builder.build();
                            let mut poly = Concrete {
                                vertices: self.vertices.clone(),
                                abs,
                            };
                            
                            println!("Faceting found");
                            poly.untangle_faces();
                            output.push(poly);
                        }
                    }

                    if let Some(max_facets) = noble {
                        if facets.len() == max_facets {
                            let t = facets.last_mut().unwrap();
                            if t.1 == possible_facets[t.0].len() - 1 {
                                t.0 += 1;
                                t.1 = 0;
                            }
                            else {
                                t.1 += 1;
                            }
                            continue
                        }
                    }
                    if irc {
                        let t = facets.last().unwrap().clone();
                        facets.push((t.0 + 1, 0));
                    } else {
                        let t = facets.last_mut().unwrap();
                        if t.1 == possible_facets[t.0].len() - 1 {
                            t.0 += 1;
                            t.1 = 0;
                        }
                        else {
                            t.1 += 1;
                        }
                    }
                }
                1 => {
                    let t = facets.last_mut().unwrap();
                    if t.1 == possible_facets[t.0].len() - 1 {
                        t.0 += 1;
                        t.1 = 0;
                    }
                    else {
                        t.1 += 1;
                    }
                }
                2 => {
                    if let Some(max_facets) = noble {
                        if facets.len() == max_facets {
                            let t = facets.last_mut().unwrap();
                            if t.1 == possible_facets[t.0].len() - 1 {
                                t.0 += 1;
                                t.1 = 0;
                            }
                            else {
                                t.1 += 1;
                            }
                            continue
                        }
                    }
                    let t = facets.last().unwrap().clone();
                    facets.push((t.0 + 1, 0));
                }
                _ => {}
            }
        }

        println!("Found {} facetings", output.len());
        return output
    }
}