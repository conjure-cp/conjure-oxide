; benchmark generated from rust API
(set-info :status unknown)
(declare-fun m2 () (Array Int Bool))
(declare-fun m () (Array Int Bool))
(assert
 (and true))
(assert
 (and true))
(assert
 (let (($x29 (not (= (select m 1) (select m2 1)))))
(or $x29)))
(check-sat)
