
git checkout gh-pages

git checkout --orphan gh-pages-clean

git rm -rf coverage/0* coverage/1* coverage/2* coverage/3* coverage/4* coverage/5* coverage/6* coverage/7* coverage/8* coverage/9* coverage/a* coverage/b* coverage/c* coverage/d* coverage/e* coverage/f*

git add -A
git commit -m "Squashing the history of gh-pages, removing past code coverage reports"

git branch -D gh-pages
git branch -m gh-pages
git push origin gh-pages --force


