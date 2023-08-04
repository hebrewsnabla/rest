use crate::scf_io::SCF;
use crate::scf_io::{vj_upper_with_ri_v, vj_full_with_ri_v,
                    vk_full_fromdm_with_ri_v};
use rest_tensors::{
    //MatFormat, ERIFull,
    MatrixFull, 
    //ERIFold4, 
    MatrixUpper, 
    //TensorSliceMut, RIFull, MatrixFullSlice, MatrixFullSliceMut
    };
use rest_tensors::{davidson_solve, DavidsonParams};
use itertools::{//Itertools, 
                iproduct, 
                //izip
                };
use tensors::MathMatrix;
use crate::utilities;
use crate::anyhow::{anyhow,Error};

impl SCF {

    pub fn stability(&mut self) -> Result< (Vec<bool>, Vec<Vec<Vec<f64>>>) ,Error> {
        // stable means the certain matrix has no negative eigenvalues
        // Table of matrix for various method
        //           internal   real2complex   external
        // Real RHF  ^1(A'+B')  ^1(A'-B')      ^3(A'+B')
        // Real UHF   A'+B'      A'-B'          A''+B''
        // See JCP 66, 3045 (1977); DOI:10.1063/1.434318
        //
        let spin_channel = self.mol.ctrl.spin_channel;
        let print_level = self.mol.ctrl.print_level;
        let params = DavidsonParams{tol:1e-4, maxspace:15, ..DavidsonParams::default()};
        let mut v_i:Vec<Vec<f64>> = vec![];
        let mut v_e:Vec<Vec<f64>> = vec![];
        let mut stable_i = true;
        let mut stable_e = true;
        let mut stable:Vec<bool> = vec![];
        let mut v:Vec<Vec<Vec<f64>>> = vec![];
        if spin_channel == 1 {
            //println!("{}", !self.is_dfa);
            //if !self.is_dfa {
                let (stable_i, v_i) = self.stability_rhf(false, &params, print_level);
                stable.push(stable_i);
                v.push(v_i);
            //}
            if !self.is_dfa {
            let (stable_e, v_e) = self.stability_rhf(true, &params, print_level);
            stable.push(stable_e);
            v.push(v_e);
            }
        }
        else {
        let mut conv:Vec<bool> = vec![];
        let mut e:Vec<f64> = vec![];
            let stable_str = ["unstable", "stable"];
            println!("Start UHF internal stability check");
            let (mut g, mut hdiag, mut h_op) = self.generate_g_hop(false).unwrap();
            let ov = g.len();
            //hdiag.iter_mut().map(|h| *h *= 2.0_f64);
            let mut x0 = vec![0.0_f64;ov];
            x0.iter_mut().zip(hdiag.iter()).for_each(|(x, hd)| //if gg.abs() > 1e-8 
                                                               {*x += 1.0 / hd});
            x0[0] += 0.2; // break symmetry of init guess
            //println!("hdiag {:?}", hdiag);
            //println!("x0 {:?}", x0);
            (conv, e, v_i) = davidson_solve(h_op, &mut x0, &mut hdiag, 
                                              &params,
                                              print_level
                           ); 
            stable_i = e[0] > -1e-5;
            println!("UHF internal : {:?}", stable_str[stable_i as usize]);
            stable.push(stable_i);
            v.push(v_i);
        }
        Ok((stable,v))

    }
    
    pub fn stability_rhf(&mut self, external: bool, 
                      params: &DavidsonParams, print_level: usize) -> (bool, Vec<Vec<f64>>) {
        let mut conv:Vec<bool> = vec![];
        let mut e:Vec<f64> = vec![];
        let mut v_i:Vec<Vec<f64>> = vec![];
        let stable_str = ["unstable", "stable"];
        let inex_str = ["internal", "external"];
        println!("Start RHF {:?} stability check", inex_str[external as usize]);
            // slow algorithm, for debug
            //self.formated_eigenvectors();
            //self.generate_h_rhf_slow();
        let (mut g, mut hdiag, mut h_op) = self.generate_g_hop(external).unwrap();
        let ov = g.len();
        //hdiag.iter_mut().map(|h| *h *= 2.0_f64);
        let mut x0 = vec![0.0_f64;ov];
        x0.iter_mut().zip(hdiag.iter()).for_each(|(x, hd)| //if gg.abs() > 1e-8 
                                                           {*x += 1.0 / hd});
        //println!("hdiag {:?}", hdiag);
        //println!("x0 {:?}", x0);
        (conv, e, v_i) = davidson_solve(h_op, &mut x0, &mut hdiag, 
                                              &params,
                                              print_level
                           ); 
        let stable_i = e[0] > -1e-5;
        println!("RHF {:?} : {:?}", inex_str[external as usize], stable_str[stable_i as usize]);
        //return Ok((stable,v));
        (stable_i, v_i)
    }

    pub fn generate_h_rhf_slow(&mut self) //-> MatrixFull<f64> 
                                          {
        let num_basis = self.mol.num_basis;
        let num_state = self.mol.num_state;
        let num_auxbas = self.mol.num_auxbas;
        //let npair = num_basis*(num_basis+1)/2;
        let spin_channel = self.mol.spin_channel;
        let mut occ = self.homo[0] + 1;
        let mut vir = num_state - occ;
        let mut ov = occ*vir;
        let mut moa = &self.eigenvectors[0];
        let mut fock = self.hamiltonian[0].to_matrixfull().unwrap();

        let mut cv = MatrixFull::from_vec([num_basis,vir],
                                          moa.iter_submatrix(0..num_basis,occ..num_basis).map(|i| *i).collect()
                                          ).unwrap();
        let mut co = MatrixFull::from_vec([num_basis,occ],
                                          moa.iter_submatrix(0..num_basis,0..occ).map(|i| *i).collect()
                                          ).unwrap();
        // foo, fvo, fvv
        let mut f_co = MatrixFull::new([num_basis, occ], 0.0_f64);
        f_co.lapack_dgemm(&mut fock.clone(), &mut co.clone(), 'N', 'N', 1.0, 0.0);
        let mut f_cv = MatrixFull::new([num_basis, vir], 0.0_f64);
        f_cv.lapack_dgemm(&mut fock.clone(), &mut cv.clone(), 'N', 'N', 1.0, 0.0);
        let mut foo = MatrixFull::new([occ,occ], 0.0_f64);
        foo.lapack_dgemm(&mut co.clone(), &mut f_co, 'T', 'N', 1.0, 0.0);
        let mut fvv = MatrixFull::new([vir,vir], 0.0_f64);
        fvv.lapack_dgemm(&mut cv.clone(), &mut f_cv, 'T', 'N', 1.0, 0.0);
        let mut fvo = MatrixFull::new([vir,occ], 0.0_f64);
        fvo.lapack_dgemm(&mut cv.clone(), &mut f_co.clone(), 'T', 'N', 1.0, 0.0);
        // g = fvo
        let mut g = fvo.data.to_vec();
        // hdiag = fvv.diag - foo.diag
        let mut hdiag = MatrixFull::new([vir, occ], 0.0_f64);
        iproduct!(fvv.get_diagonal_terms().unwrap().iter(),
                  foo.get_diagonal_terms().unwrap().iter()).map(|(v,o)| {(*v,*o)})
            .zip(hdiag.data.iter_mut()).for_each(|(vo,h)| {
                  *h = vo.0 - vo.1
            });
        let mut hdiagvec = hdiag.data.to_vec();
        let mut amat = MatrixFull::new([ov,ov], 0.0_f64);
        let mut bmat = MatrixFull::new([ov,ov], 0.0_f64);
        for ia in 0..ov {
            amat.data[ia*ov+ia] = hdiagvec[ia]
        }

        //let mut vk: Vec<MatrixFull<f64>> = vec![MatrixFull::new([1,1],0.0f64),MatrixFull::new([1,1],0.0f64)];
        if let Some(ri3fn) = &self.ri3fn {
            //let num_basis = ri3fn.size[0];
            //let num_auxbas = ri3fn.size[2];
            //let npair = num_basis*(num_basis+1)/2;
            for i_spin in (0..spin_channel) {
                //let mut tmp_mu = vec![0.0f64;num_auxbas];
                //let mut vk_spin = &mut vk[i_spin];
                //*vk_spin = MatrixFull::new([num_basis, num_basis],0.0f64);
                
                ri3fn.iter_auxbas(0..num_auxbas).unwrap().enumerate().for_each(|(i,m)| {
                    //prepare L_{ia}^{\mu} = \sum_{kl} Co_{ki} * M_{kl}^{\mu} * Cv_{la} 
                    //        L_{ab}^{\mu} = \sum_{kl} Co_{ka} * M_{kl}^{\mu} * Cv_{lb} 
                    //        L_{ij}^{\mu} = \sum_{kl} Co_{ki} * M_{kl}^{\mu} * Cv_{lj} 
                    let mut tmp_mu = MatrixFull::from_vec([num_basis, num_basis], m.to_vec()).unwrap();
                    let mut m_cv = MatrixFull::new([num_basis, vir], 0.0f64);
                    m_cv.lapack_dgemm(&mut tmp_mu, &mut cv.clone(), 'N', 'N', 1.0, 0.0);
                    let mut m_co = MatrixFull::new([num_basis, occ], 0.0f64);
                    m_co.lapack_dgemm(&mut tmp_mu, &mut co.clone(), 'N', 'N', 1.0, 0.0);
                    let mut l_ov = MatrixFull::new([occ, vir], 0.0f64);
                    l_ov.lapack_dgemm(&mut co.clone(), &mut m_cv, 'T', 'N', 1.0, 0.0);
                    let mut l_vv = MatrixFull::new([vir,vir], 0.0f64);
                    l_vv.lapack_dgemm(&mut cv.clone(), &mut m_cv, 'T', 'N', 1.0, 0.0);
                    let mut l_oo = MatrixFull::new([occ, occ], 0.0f64);
                    l_oo.lapack_dgemm(&mut co.clone(), &mut m_co, 'T', 'N', 1.0, 0.0);
                    let mut l_ov_vec = l_ov.data.to_vec();
                    let mut l_vv_vec = l_vv.data.to_vec();
                    let mut l_oo_vec = l_oo.data.to_vec();
                    for ia in 0..ov {
                        for jb in 0..ov {
                            amat.data[jb*ov+ia] += l_ov_vec[ia]*l_ov_vec[jb]*2.0;
                            bmat.data[jb*ov+ia] += l_ov_vec[ia]*l_ov_vec[jb]*2.0;
                        }
                    }
                    for i in 0..occ {
                        for j in 0.. occ {
                            for a in 0..vir {
                                for b in 0..vir {
                                    let ia = a*occ+i;
                                    let jb = b*occ+j;
                                    let ab = b*vir+a;
                                    let ij = j*occ+i;
                                    let ib = b*occ+i;
                                    let ja = a*occ+j;
                                    amat.data[jb*ov+ia] -= l_vv_vec[ab]*l_oo_vec[ij];
                                    bmat.data[jb*ov+ia] -= l_ov_vec[ja]*l_ov_vec[ib];
                                }
                            }
                        }
                    }
    
                    // fill vk[i_spin] with the contribution from the given {\mu}:
                    //
                    // \sum_j M_{ij}^{\mu} * (\sum_{l}D_{jl}*M_{kl}^{\mu})
                    //
                    //vk_spin.lapack_dgemm(&mut tmp_mu.clone(), &mut dm_m, 'N', 'N', 1.0, 1.0);
                });
            }

            amat.formated_output(ov, "full");
            bmat.formated_output(ov, "full");
            let mut a_plus_b = amat.clone();
            a_plus_b.self_add(&mut bmat);
            let mut apb_upper = a_plus_b.clone().to_matrixupper();
            let (mut v, mut e, n) = apb_upper.to_matrixupperslicemut().lapack_dspevx().unwrap();
            println!("eigval {:?}", e.to_vec());
        };

    }
    pub fn generate_g_hop(&mut self, external: bool) -> Result<(Vec<f64>, Vec<f64>, 
                                                                    Box<dyn FnMut(&Vec<f64>) -> Vec<f64> + '_>),
                                                                    Error
                                                                  > {
        let num_basis = self.mol.num_basis;
        let num_state = self.mol.num_state;
        let num_auxbas = self.mol.num_auxbas;
        //let npair = num_basis*(num_basis+1)/2;
        let mut _occ:Vec<usize> = vec![0, 0];
        let mut _vir:Vec<usize> = vec![0, 0];
        let mut ov:Vec<usize> = vec![0, 0];
        let spin_channel = self.mol.spin_channel;
        let mut cv = [MatrixFull::empty(), MatrixFull::empty()];
        let mut co = [MatrixFull::empty(), MatrixFull::empty()];
        let mut foo = [MatrixFull::empty(), MatrixFull::empty()];
        let mut fvv = [MatrixFull::empty(), MatrixFull::empty()];
        let mut fvo = [MatrixFull::empty(), MatrixFull::empty()];
        let mut g = vec![vec![], vec![]];
        let mut hdiagvec = vec![vec![], vec![]];
        let mut factor = 1.0;
        if spin_channel == 1 {
            factor = 4.0;
        } else {
            factor = 2.0;
        }
        for ispin in 0..spin_channel {
            _occ[ispin] = self.homo[ispin] + 1;
            //println!("{:?}", _occ);
            _vir[ispin] = num_state - _occ[ispin];
            ov[ispin] = _occ[ispin]*_vir[ispin];
            let occ = _occ[ispin];
            let vir = _vir[ispin];
            let mut mo = &self.eigenvectors[ispin];
            let mut fock = self.hamiltonian[ispin].to_matrixfull().unwrap();

            cv[ispin] = MatrixFull::from_vec([num_basis,vir],
                                          mo.iter_submatrix(0..num_basis,occ..num_basis).map(|i| *i).collect()
                                          ).unwrap();
            co[ispin] = MatrixFull::from_vec([num_basis,occ],
                                          mo.iter_submatrix(0..num_basis,0..occ).map(|i| *i).collect()
                                          ).unwrap();
            // foo, fvo, fvv
            let mut f_co = MatrixFull::new([num_basis, occ], 0.0_f64);
            f_co.lapack_dgemm(&mut fock.clone(), &mut co[ispin].clone(), 'N', 'N', 1.0, 0.0);
            let mut f_cv = MatrixFull::new([num_basis, vir], 0.0_f64);
            f_cv.lapack_dgemm(&mut fock.clone(), &mut cv[ispin].clone(), 'N', 'N', 1.0, 0.0);
            foo[ispin] = MatrixFull::new([occ,occ], 0.0_f64);
            foo[ispin].lapack_dgemm(&mut co[ispin].clone(), &mut f_co, 'T', 'N', 1.0, 0.0);
            fvv[ispin] = MatrixFull::new([vir,vir], 0.0_f64);
            fvv[ispin].lapack_dgemm(&mut cv[ispin].clone(), &mut f_cv, 'T', 'N', 1.0, 0.0);
            fvo[ispin] = MatrixFull::new([vir,occ], 0.0_f64);
            fvo[ispin].lapack_dgemm(&mut cv[ispin].clone(), &mut f_co.clone(), 'T', 'N', 1.0, 0.0);
            // g = fvo
            g[ispin] = fvo[ispin].data.to_vec();
            // hdiag = fvv.diag - foo.diag
            let mut hdiag = MatrixFull::new([vir, occ], 0.0_f64);
            iproduct!(fvv[ispin].get_diagonal_terms().unwrap().iter(),
                      foo[ispin].get_diagonal_terms().unwrap().iter()).map(|(v,o)| {(*v,*o)})
                .zip(hdiag.data.iter_mut()).for_each(|(vo,h)| {
                      *h = vo.0 - vo.1
                });
            hdiagvec[ispin] = hdiag.data.to_vec();
        }
        let mut g_all = g.concat();
        let mut hdiag_all = hdiagvec.concat();
        if !external {
            hdiag_all.iter_mut().for_each(|h| *h *= factor);
        }
        let mut xfac = 1.0;
        if spin_channel == 1 { xfac *= 2.0; }
        let h_op = move |xvec:&Vec<f64>| -> Vec<f64> {
            let mut xmat = [MatrixFull::empty(), MatrixFull::empty()];
            let mut sigma = vec![MatrixFull::empty(), MatrixFull::empty()];
            let mut sigmavec = [vec![], vec![]];
            let mut d1 = vec![MatrixFull::empty(), MatrixFull::empty()];

            for ispin in 0..spin_channel {
                let xstart = 0 + ov[0]*ispin;
                let xend = ov[0] + ov[1]*ispin;
                let occ = _occ[ispin];
                let vir = _vir[ispin];
                //println!("{:?} {:?}", xstart, xend);
                xmat[ispin] = MatrixFull::from_vec([vir, occ], xvec[xstart..xend].to_vec()).unwrap();
                sigma[ispin] = MatrixFull::new([vir, occ], 0.0_f64);
                // sigma = F_vv . x - x . F_oo
                sigma[ispin].lapack_dgemm(&mut fvv[ispin].clone(), &mut xmat[ispin], 'N', 'N', 1.0 ,0.0);
                sigma[ispin].lapack_dgemm( &mut xmat[ispin].clone(), &mut foo[ispin].clone(), 'N', 'N', -1.0 ,1.0);

                let mut x_co = MatrixFull::new([vir, num_basis], 0.0);
                x_co.lapack_dgemm(&mut xmat[ispin].clone(), &mut co[ispin].clone(), 'N', 'T', 1.0, 0.0);
                d1[ispin] = MatrixFull::new([num_basis, num_basis], 0.0);
                d1[ispin].lapack_dgemm(&mut cv[ispin].clone(), &mut x_co, 'N', 'N', xfac, 0.0);
                let mut d1_t = d1[ispin].transpose();
                d1[ispin].self_add( &mut d1_t );
                //d1[ispin].formated_output(num_basis, "full");            
            }
            let mut vind = self.response_fn(&mut d1, external);
            for ispin in 0..spin_channel {
                let occ = _occ[ispin];
                let vir = _vir[ispin];
                // sigma += Cv^T . vind . Co
                let mut v_co = MatrixFull::new([num_basis, occ], 0.0);
                v_co.lapack_dgemm(&mut vind[ispin], &mut co[ispin].clone(), 'N', 'N', 1.0, 0.0);
                sigma[ispin].lapack_dgemm(&mut cv[ispin].clone(), &mut v_co.clone(), 'T', 'N', 1.0, 1.0);
                //println!("vind {:?}", vind[ispin]);
                //println!("sigma {:?}", sigma);
                sigmavec[ispin] = sigma[ispin].data.to_vec();
            }
            let mut sigma_all = sigmavec.concat();
            if !external {
                sigma_all.iter_mut().for_each(|s| *s *= factor);
            }

            sigma_all
        };
        Ok((g_all, hdiag_all, Box::new(h_op)))
    }

    //pub fn generate_g_hop_uhf(&mut self, external: bool) -> Result<(Vec<f64>, Vec<f64>, 
    //                                                                Box<dyn FnMut(Vec<f64>) -> Vec<f64> + '_>),
    //                                                                Error
    //                                                              > {
    //    //if external {
    //        return Err(anyhow!("UHF->GHF stability not implemented"))
    //    //}
    //}
    pub fn response_fn(&mut self, dm: &mut Vec<MatrixFull<f64>>,
        //scaling_factor: f64,
        external: bool
        ) -> Vec<MatrixFull<f64>> {
        let mut hyb = 1.0f64;
        if self.is_dfa {
            hyb = self.mol.xc_data.dfa_hybrid_scf;
        }
        let mut vind = self.response_fn_hf(dm, external, hyb);
        if self.is_dfa {
            let mut vind_dfa = self.response_fn_dfa(dm, external);
            let spin_channel = self.mol.ctrl.spin_channel;
            for ispin in 0..spin_channel {
                vind[ispin].self_scaled_add(&vind_dfa[ispin], 1.0f64);
            }
        }
        vind
    }
    
    pub fn response_fn_hf(&mut self, dm: &mut Vec<MatrixFull<f64>>,
                                      //scaling_factor: f64,
                                      external: bool,
                                      hyb: f64
                                      ) -> Vec<MatrixFull<f64>> {
        if self.mol.ctrl.spin_channel == 1 {
            if external {
                let mut vind = self.response_vk_full_with_ri_v( dm, -0.5*hyb);
                vind
            } else {
                // vj - 0.5*vk
                let mut vj = self.response_vj_full_with_ri_v( dm, 1.0);
                let mut vind = vj.clone();
                if hyb != 0.0f64 {
                    let mut vk = self.response_vk_full_with_ri_v( dm, 1.0);
                    vind[0] = vj[0].scaled_add(&vk[0], -0.5*hyb).unwrap();
                }
                vind
            }
        } else {
            if external {
                let mut vind = self.response_vk_full_with_ri_v( dm, -1.0*hyb);
                vind
            } else {
                // vind[ispin] = vj[alpha] + vj[beta] - vk[ispin]
                let mut vj = self.response_vj_full_with_ri_v( dm, 1.0);
                let mut vind = vj.clone();
                vind[0].self_add(&vj[1]);
                vind[1].self_add(&vj[0]);
                if hyb != 0.0f64 {
                    let mut vk = self.response_vk_full_with_ri_v( dm, -1.0*hyb);
                    vind[0].self_add(&vk[0]);
                    vind[1].self_add(&vk[1]);
                }
                //vj[0].formated_output(vj[0].size[0], "full");            
                //vj[1].formated_output(vj[0].size[0], "full");            
                //vk[0].formated_output(vj[0].size[0], "full");            
                //vk[1].formated_output(vj[0].size[0], "full");            
                vind
            }

        }
    }

    pub fn response_fn_dfa(&mut self, dm: &mut Vec<MatrixFull<f64>>,
        //scaling_factor: f64,
        external: bool
        ) -> Vec<MatrixFull<f64>> {
        let mut dm0 = self.density_matrix.clone();
        //rho, vxc, fxc = self.transformed_vxc_fxc();
        if self.mol.ctrl.spin_channel == 1 {

            println!("response_fn_dfa for {}", external);
            if external {
                let mut vind = self.response_fxc_st(&mut dm0, dm);
                vind[0].self_multiple(0.5f64);
                vind
            } else {
                let mut vind = self.response_fxc(&mut dm0, dm);
                vind
            }

        } else {

        panic!("not implemented");

        }
    }



    pub fn response_fxc_st(&mut self, dm0: &mut Vec<MatrixFull<f64>>,
                           dm: &mut Vec<MatrixFull<f64>>
                        ) -> Vec<MatrixFull<f64>> {
        panic!("not implemented");
        // should call response_fxc
    }

    pub fn response_fxc(&mut self, dm0: &mut Vec<MatrixFull<f64>>,
                           dm: &mut Vec<MatrixFull<f64>>
                        ) -> Vec<MatrixFull<f64>> {
        let mut vmat = vec![];
        let spin_channel = self.mol.ctrl.spin_channel;
        if spin_channel == 1 {
            if let Some(grids) = &mut self.grids {
                //vmat = self.mol.xc_data.xc_fxc_vmat(grids, spin_channel, dm0, dm, 3);
                vmat = self.generate_fxc(dm0, dm, 1.0f64);
            }
        }
        vmat

    }


    pub fn generate_fxc(&mut self, dm0: &mut Vec<MatrixFull<f64>>,
                        dm: &mut Vec<MatrixFull<f64>>, 
                        scaling_factor: f64) -> Vec<MatrixFull<f64>> {
        let num_basis = self.mol.num_basis;
        let num_state = self.mol.num_state;
        let num_auxbas = self.mol.num_auxbas;
        let npair = num_basis*(num_basis+1)/2;
        let spin_channel = self.mol.spin_channel;
        //let mut vxc: MatrixUpper<f64> = MatrixUpper::new(1,0.0f64);
        //let mut fxc:Vec<MatrixFull<f64>> = vec![MatrixFull::empty();spin_channel];
        //let mut exc_spin:Vec<f64> = vec![];
        //let mut exc_total:f64 = 0.0;
        let mut fxc_mf:Vec<MatrixFull<f64>> = vec![MatrixFull::empty();spin_channel];
        //let dm = &mut self.density_matrix;
        let mo = &mut self.eigenvectors;
        let occ = &mut self.occupation;
        let print_level = self.mol.ctrl.print_level;
        if let Some(grids) = &mut self.grids {
            let dt0 = utilities::init_timing();
            let mut fxc_ao = self.mol.xc_data.xc_fxc_ao(grids, spin_channel, dm0, dm, print_level);
            let dt1 = utilities::timing(&dt0, Some("Total fxc_ao time"));
            //exc_spin = exc;
            if let Some(ao) = &mut grids.ao {
                // Evaluate the exchange-correlation energy
                //exc_total = izip!(grids.weights.iter(),exc.data.iter()).fold(0.0,|acc,(w,e)| {
                //    acc + w*e
                //});
                for i_spin in 0..spin_channel {
                    let fxc_mf_s = fxc_mf.get_mut(i_spin).unwrap();
                    *fxc_mf_s = MatrixFull::new([num_basis,num_basis],0.0f64);
                    let fxc_ao_s = fxc_ao.get_mut(i_spin).unwrap();
                    fxc_mf_s.lapack_dgemm(ao, fxc_ao_s, 'N', 'T', 1.0, 0.0);
                    
                }
            }
            let dt2 = utilities::timing(&dt1, Some("From fxc_ao to fxc"));
        }

        println!("fxc_mf {:?}", fxc_mf);

        //let dt0 = utilities::init_timing();
        //for i_spin in (0..spin_channel) {
        //    let mut fxc_s = fxc.get_mut(i_spin).unwrap();
        //    let mut fxc_mf_s = fxc_mf.get_mut(i_spin).unwrap();
        //
        //    fxc_mf_s.self_add(&fxc_mf_s.transpose());
        //    fxc_mf_s.self_multiple(0.5);
        //    //println!("debug vxc{}",i_spin);
        //    //vxc_mf_s.formated_output(10, "full");
        //    *fxc_s = fxc_mf_s.to_matrixupper();
        //}
        //
        //utilities::timing(&dt0, Some("symmetrize fxc"));

        //exc_total = exc_spin.iter().sum();


        if scaling_factor!=1.0f64 {
            //exc_total *= scaling_factor;
            for i_spin in (0..spin_channel) {
                fxc_mf[i_spin].data.iter_mut().for_each(|f| *f = *f*scaling_factor)
            }
        };

        fxc_mf

    }

    // Currently response fn can only be calculated with ri
    //
    pub fn response_vj_full_with_ri_v(&mut self, dm: &Vec<MatrixFull<f64>>,
                                      scaling_factor: f64) -> Vec<MatrixFull<f64>> {
        let spin_channel = self.mol.spin_channel;
        //let dm = &self.density_matrix;

        vj_full_with_ri_v(&self.ri3fn, dm, spin_channel, scaling_factor)
        // alternative way:
        //let mut vj = vec![MatrixFull::new([1, 1], 0.0), MatrixFull::empty()];
        //let mut vju = vj_upper_with_ri_v(&self.ri3fn, dm, spin_channel, scaling_factor);
        //vj[0] = vju[0].to_matrixfull().unwrap();
        //vj
    }
    pub fn response_vj_upper_with_ri_v(&mut self, dm: &Vec<MatrixFull<f64>>,
                                      scaling_factor: f64) -> Vec<MatrixUpper<f64>> {
        let spin_channel = self.mol.spin_channel;
        //let dm = &self.density_matrix;

        vj_upper_with_ri_v(&self.ri3fn, dm, spin_channel, scaling_factor)
    }
    pub fn response_vk_full_with_ri_v(&mut self, dm: &Vec<MatrixFull<f64>>,
                                      scaling_factor: f64) -> Vec<MatrixFull<f64>> {
        let spin_channel = self.mol.spin_channel;
        //let dm = &self.density_matrix;
        if scaling_factor != 0.0f64 {
            vk_full_fromdm_with_ri_v(&self.ri3fn, dm, spin_channel, scaling_factor)
        } else {
            let num_basis = dm[0].size[0];
            let zerovk = MatrixFull::new([num_basis,num_basis],0.0f64);
            let mut vk = vec![zerovk.clone(), MatrixFull::empty()];
            if spin_channel == 2 { vk[1] = zerovk.clone()}
            vk
        }
    }

}


