-- an algebraic data type of either bool or bool bool
data X = L Bool | R Bool Bool


f :: X -> Bool 
f x = case x of
  L True -> True
  R False _ -> True
  otherwise -> False 

-- so what is a nice way to 1. model this in essence abstractly, and 
-- 2. make the intermediary in a way that is easy to work with and agnostic to the actual implementation of the data type.
-- t(x)? = L \/ R
-- u(x)? L <-> True \/ False
-- u(x)? R <-> True False \/ True True
-- f(x)? = u(x)? L -> True \/ u(x)? R -> False _
